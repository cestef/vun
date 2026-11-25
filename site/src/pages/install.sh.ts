// The repository's installer, served as text for a reader to download and read before running it;
// the page never asks anyone to pipe it into a shell.

import {source} from '../../build/source';

export const GET = () => new Response(source('install.sh'), {headers: {'Content-Type': 'text/plain; charset=utf-8'}});
