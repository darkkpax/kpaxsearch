import { it, expect } from 'vitest';
import { parsePreferences } from './preferences';

it('defaults to acrylic, rounded corners and a light overlay', () => expect(parsePreferences(null)).toEqual({blur:true,material:'acrylic',radius:16,density:25,rows:7}));
it('clamps persisted values to supported ranges', () => expect(parsePreferences('{"blur":false,"density":999,"rows":-1,"radius":999}')).toEqual({blur:false,material:'acrylic',radius:40,density:100,rows:4}));
it('recovers malformed preferences', () => expect(parsePreferences('[]')).toEqual(parsePreferences(null)));
it('rejects wrong types without coercion', () => expect(parsePreferences('{"blur":"false","density":"90","rows":null}')).toEqual(parsePreferences(null)));
it('rejects unsupported effects', () => expect(parsePreferences('{"material":"unknown"}').material).toBe('acrylic'));
it('accepts alternate materials and square corners', () => expect(parsePreferences('{"material":"blur","radius":0}')).toMatchObject({material:'blur',radius:0}));
