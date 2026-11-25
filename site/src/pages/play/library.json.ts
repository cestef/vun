import {library} from '../../../build/compiler';

export const GET = () => Response.json(library());
