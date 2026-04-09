import { defineCollection, z } from "astro:content";

const books = defineCollection({
  type: "content",
  schema: z.object({
    title: z.string(),
    hall: z.string(),
    hallSlug: z.string(),
    shelf: z.string(),
    catalogId: z.string(),
    value: z.string(),
    rarity: z.string(),
    status: z.string(),
    quote: z.string(),
    anchor: z.string(),
    catalogDate: z.string(),
    lastEdit: z.string(),
  }),
});

export const collections = { books };
