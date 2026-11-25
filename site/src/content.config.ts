import {docsLoader} from '@astrojs/starlight/loaders';
import {docsSchema} from '@astrojs/starlight/schema';
import {z} from 'astro/zod';
import {defineCollection} from 'astro:content';

// A page with a `lesson` has an editor under its text: what to make the program do, a hint, and
// how a run must end (`output` the program's, `exit` 0 where unsaid, or `error` the compiler's
// refusal). A lesson with no `expect` is free play.
const lesson = z.object({
  task: z.string().optional(),
  hint: z.string().optional(),
  expect: z.object({output: z.string().optional(), error: z.string().optional(), exit: z.number().optional()}).optional(),
});

export const collections = {docs: defineCollection({loader: docsLoader(), schema: docsSchema({extend: z.object({lesson: lesson.optional()})})})};
