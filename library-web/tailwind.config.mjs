/** @type {import('tailwindcss').Config} */
export default {
  content: ["./src/**/*.{astro,html,js,jsx,md,mdx,svelte,ts,tsx,vue}"],
  theme: {
    extend: {
      colors: {
        parchment: "#F5F0E8",
        "parchment-dark": "#E8DFD0",
        ink: "#3A3226",
        "ink-light": "#6B5D4F",
        cinnabar: "#8B2500",
        "cinnabar-light": "#A63A1A",
        gold: "#C5A258",
        "gold-dim": "#9E8245",
        jade: "#5B8C6F",
      },
      fontFamily: {
        kai: ['"LXGW WenKai"', "KaiTi", "STKaiti", "serif"],
        song: ['"Noto Serif SC"', "SimSun", "serif"],
      },
    },
  },
  plugins: [],
};
