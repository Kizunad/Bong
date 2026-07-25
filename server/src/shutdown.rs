//! OS shutdown signal bridge.
//!
//! The listener lives entirely outside ECS: a named std thread owns a dedicated
//! Tokio runtime and forwards SIGINT/SIGTERM (or Ctrl-C on non-Unix platforms)
//! through a bounded channel. ECS observes that channel in `PreUpdate`, emits one
//! `AppExit::Success`, and leaves `Last` systems in the same frame to persist
//! dirty state before the app runner returns.

use std::sync::mpsc;
use std::thread;

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use valence::prelude::{bevy_ecs, App, AppExit, EventWriter, PreUpdate, ResMut, Resource};

/// The OS interruption source forwarded by the listener thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownSignal {
    Interrupt,
    Terminate,
    CtrlC,
}

/// Receiver owned by ECS; the producer remains isolated in the listener thread.
#[derive(Resource)]
pub struct ShutdownSignalReceiver {
    receiver: Receiver<ShutdownSignal>,
    exit_emitted: bool,
}

impl ShutdownSignalReceiver {
    #[cfg(test)]
    fn for_test(receiver: Receiver<ShutdownSignal>) -> Self {
        Self {
            receiver,
            exit_emitted: false,
        }
    }
}

/// Installs the production listener and its `PreUpdate` bridge.
///
/// Listener initialization is acknowledged before this function returns. A
/// failure to create the Tokio runtime or install a signal stream aborts startup
/// instead of silently running a server that cannot flush on normal shutdown.
pub fn register(app: &mut App) {
    let receiver = spawn_listener_or_panic();
    register_with_receiver(app, receiver);
}

fn register_with_receiver(app: &mut App, receiver: Receiver<ShutdownSignal>) {
    app.add_event::<AppExit>();
    app.insert_resource(ShutdownSignalReceiver {
        receiver,
        exit_emitted: false,
    });
    app.add_systems(PreUpdate, emit_app_exit_on_shutdown_signal);
}

fn emit_app_exit_on_shutdown_signal(
    mut shutdown: ResMut<ShutdownSignalReceiver>,
    mut app_exit: EventWriter<AppExit>,
) {
    if shutdown.exit_emitted {
        return;
    }

    if shutdown.receiver.try_recv().is_ok() {
        shutdown.exit_emitted = true;
        app_exit.send(AppExit::Success);
    }
}

fn spawn_listener_or_panic() -> Receiver<ShutdownSignal> {
    let (sender, receiver) = bounded(1);
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);

    thread::Builder::new()
        .name("bong-shutdown-signal-listener".to_owned())
        .spawn(move || run_listener_thread(sender, ready_sender))
        .unwrap_or_else(|error| panic!("failed to spawn shutdown signal listener: {error}"));

    match ready_receiver.recv() {
        Ok(Ok(())) => receiver,
        Ok(Err(error)) => panic!("failed to initialize shutdown signal listener: {error}"),
        Err(error) => panic!("shutdown signal listener exited before readiness: {error}"),
    }
}

/// Forwards at most one pending signal to ECS; the next frame emits the single
/// app exit event. A disconnected receiver means the application has already
/// gone away, so the listener exits rather than spinning indefinitely.
fn forward_shutdown_signal_to_app_exit(
    sender: &Sender<ShutdownSignal>,
    signal: ShutdownSignal,
) -> Result<(), ()> {
    match sender.try_send(signal) {
        Ok(()) | Err(TrySendError::Full(_)) => Ok(()),
        Err(TrySendError::Disconnected(_)) => Err(()),
    }
}

#[cfg(any(not(unix), test))]
fn handle_initial_ctrl_c_poll(
    sender: &Sender<ShutdownSignal>,
    ready: &mpsc::SyncSender<Result<(), String>>,
    initial_poll: std::task::Poll<Result<(), std::io::Error>>,
) -> bool {
    match initial_poll {
        std::task::Poll::Ready(Ok(())) => {
            if ready.send(Ok(())).is_ok() {
                let _ = forward_shutdown_signal_to_app_exit(sender, ShutdownSignal::CtrlC);
            }
            false
        }
        std::task::Poll::Ready(Err(error)) => {
            let _ = ready.send(Err(format!("install Ctrl-C handler: {error}")));
            false
        }
        std::task::Poll::Pending => ready.send(Ok(())).is_ok(),
    }
}

#[cfg(unix)]
fn run_listener_thread(
    sender: Sender<ShutdownSignal>,
    ready: mpsc::SyncSender<Result<(), String>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready.send(Err(format!("build Tokio runtime: {error}")));
            return;
        }
    };

    runtime.block_on(async move {
        use tokio::signal::unix::{signal, SignalKind};

        let mut interrupt = match signal(SignalKind::interrupt()) {
            Ok(stream) => stream,
            Err(error) => {
                let _ = ready.send(Err(format!("install SIGINT handler: {error}")));
                return;
            }
        };
        let mut terminate = match signal(SignalKind::terminate()) {
            Ok(stream) => stream,
            Err(error) => {
                let _ = ready.send(Err(format!("install SIGTERM handler: {error}")));
                return;
            }
        };

        if ready.send(Ok(())).is_err() {
            return;
        }

        loop {
            let signal = tokio::select! {
                received = interrupt.recv() => match received {
                    Some(()) => ShutdownSignal::Interrupt,
                    None => {
                        tracing::error!("[bong][shutdown] SIGINT stream closed; listener thread exits");
                        return;
                    }
                },
                received = terminate.recv() => match received {
                    Some(()) => ShutdownSignal::Terminate,
                    None => {
                        tracing::error!("[bong][shutdown] SIGTERM stream closed; listener thread exits");
                        return;
                    }
                },
            };
            if forward_shutdown_signal_to_app_exit(&sender, signal).is_err() {
                return;
            }
        }
    });
}

#[cfg(not(unix))]
fn run_listener_thread(
    sender: Sender<ShutdownSignal>,
    ready: mpsc::SyncSender<Result<(), String>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready.send(Err(format!("build Tokio runtime: {error}")));
            return;
        }
    };

    runtime.block_on(async move {
        // `tokio::signal::ctrl_c()` installs its handler when its future is first
        // polled. Preserve that poll's result: if Ctrl-C has already arrived, the
        // future is complete and must not be awaited a second time.
        let mut ctrl_c = Box::pin(tokio::signal::ctrl_c());
        let initial_poll = std::future::poll_fn(|context| {
            std::task::Poll::Ready(std::future::Future::poll(ctrl_c.as_mut(), context))
        })
        .await;
        if !handle_initial_ctrl_c_poll(&sender, &ready, initial_poll) {
            return;
        }
        match ctrl_c.await {
            Ok(()) => {
                let _ = forward_shutdown_signal_to_app_exit(&sender, ShutdownSignal::CtrlC);
            }
            Err(error) => tracing::error!("[bong][shutdown] Ctrl-C listener failed: {error}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{EventReader, Last, Resource};

    #[derive(Resource, Default)]
    struct ExitEventsSeen(usize);

    fn observe_exit_in_last(mut seen: ResMut<ExitEventsSeen>, mut exits: EventReader<AppExit>) {
        seen.0 += exits.read().count();
    }

    fn test_app(receiver: Receiver<ShutdownSignal>) -> App {
        let mut app = App::new();
        app.add_event::<AppExit>();
        app.insert_resource(ShutdownSignalReceiver::for_test(receiver));
        app.add_systems(PreUpdate, emit_app_exit_on_shutdown_signal);
        app
    }

    #[test]
    fn empty_receiver_does_not_emit_exit() {
        let (_sender, receiver) = bounded(1);
        let mut app = test_app(receiver);
        app.update();
        assert!(
            app.should_exit().is_none(),
            "empty receiver must not exit the app"
        );
    }

    #[test]
    fn interrupt_and_terminate_each_emit_success() {
        for signal in [ShutdownSignal::Interrupt, ShutdownSignal::Terminate] {
            let (sender, receiver) = bounded(1);
            sender.send(signal).expect("test receiver must be open");
            let mut app = test_app(receiver);
            app.update();
            assert_eq!(
                app.should_exit(),
                Some(AppExit::Success),
                "{signal:?} must become AppExit::Success"
            );
        }
    }

    #[test]
    fn repeated_signals_are_coalesced_to_one_exit() {
        let (sender, receiver) = crossbeam_channel::unbounded();
        sender.send(ShutdownSignal::Interrupt).unwrap();
        sender.send(ShutdownSignal::Terminate).unwrap();
        let mut app = test_app(receiver);
        app.insert_resource(ExitEventsSeen::default());
        app.add_systems(Last, observe_exit_in_last);
        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<ExitEventsSeen>().0,
            1,
            "multiple OS signals must produce one AppExit event"
        );
    }

    #[test]
    fn last_observes_exit_emitted_from_preupdate_in_same_frame() {
        let (sender, receiver) = bounded(1);
        sender.send(ShutdownSignal::Interrupt).unwrap();
        let mut app = test_app(receiver);
        app.insert_resource(ExitEventsSeen::default());
        app.add_systems(Last, observe_exit_in_last);
        app.update();
        assert_eq!(
            app.world().resource::<ExitEventsSeen>().0,
            1,
            "Last must see the PreUpdate exit in the shutdown frame"
        );
    }

    #[test]
    fn initial_ctrl_c_poll_routes_pending_ready_and_error_without_repolling() {
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let (sender, receiver) = bounded(1);
        assert!(handle_initial_ctrl_c_poll(
            &sender,
            &ready_sender,
            std::task::Poll::Pending,
        ));
        assert!(matches!(ready_receiver.recv(), Ok(Ok(()))));
        assert!(
            receiver.try_recv().is_err(),
            "pending Ctrl-C must await a future signal"
        );

        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let (sender, receiver) = bounded(1);
        assert!(!handle_initial_ctrl_c_poll(
            &sender,
            &ready_sender,
            std::task::Poll::Ready(Ok(())),
        ));
        assert!(matches!(ready_receiver.recv(), Ok(Ok(()))));
        assert_eq!(
            receiver.try_recv(),
            Ok(ShutdownSignal::CtrlC),
            "an already-complete Ctrl-C future must be forwarded without a second poll"
        );

        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let (sender, receiver) = bounded(1);
        assert!(!handle_initial_ctrl_c_poll(
            &sender,
            &ready_sender,
            std::task::Poll::Ready(Err(std::io::Error::other("listener unavailable"))),
        ));
        assert!(
            ready_receiver.recv().is_ok_and(|result| result.is_err()),
            "Ctrl-C setup failure must reject readiness"
        );
        assert!(
            receiver.try_recv().is_err(),
            "failed Ctrl-C setup must not emit shutdown"
        );
    }

    #[test]
    fn register_with_receiver_pins_bridge_to_preupdate() {
        fn schedule_contains_system(
            app: &App,
            schedule: impl valence::prelude::bevy_ecs::schedule::ScheduleLabel,
            expected_name: &str,
        ) -> bool {
            app.get_schedule(schedule).is_some_and(|schedule| {
                schedule
                    .graph()
                    .systems()
                    .any(|(_, system, _)| system.name().as_ref() == expected_name)
            })
        }

        let mut app = App::new();
        let (_sender, receiver) = bounded(1);
        register_with_receiver(&mut app, receiver);

        let bridge = std::any::type_name_of_val(&emit_app_exit_on_shutdown_signal);
        assert!(
            schedule_contains_system(&app, PreUpdate, bridge),
            "shutdown::register must pin `{bridge}` into PreUpdate so Last flush hooks observe AppExit in the same frame"
        );
    }
}
