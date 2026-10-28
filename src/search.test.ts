import { describe, it, expect } from 'vitest';
import { search, type Program } from './search';

const apps: Program[] = [
  { id: 'chrome', name: 'Google Chrome', icon: '' },
  { id: 'code', name: 'Visual Studio Code', icon: '' },
  { id: 'studio', name: 'Visual Studio', icon: '' },
  { id: 'discord', name: 'Discord', icon: '' },
  { id: 'calc', name: 'Калькулятор', icon: '' },
];
describe('program search', () => {
  it('finds word prefixes regardless of case', () => expect(search(apps, 'CHRO')[0]?.id).toBe('chrome'));
  it('ranks exact names before partial names', () => expect(search(apps, 'visual studio')[0]?.id).toBe('studio'));
  it('finds initials', () => expect(search(apps, 'vsc')[0]?.id).toBe('code'));
  it('finds ordered fuzzy characters', () => expect(search(apps, 'dscord')[0]?.id).toBe('discord'));
  it('corrects russian keyboard input for latin names', () => expect(search(apps, 'вшы')[0]?.id).toBe('discord'));
  it('corrects latin keyboard input for russian names', () => expect(search(apps, 'rfkmr')[0]?.id).toBe('calc'));
  it('returns no unrelated applications', () => expect(search(apps, 'zzzzzz')).toEqual([]));
  it('puts recent launches first for an empty query', () => expect(search(apps, '  ', ['discord'])[0]?.id).toBe('discord'));
  it('does not let history outrank a more exact match', () => expect(search(apps, 'visual studio', ['code'])[0]?.id).toBe('studio'));
  it('accepts search words in any order', () => expect(search(apps, 'code visual')[0]?.id).toBe('code'));
  it('puts frequent launches ahead of a more recent single launch', () => expect(search(apps, '', ['discord'], {code: 12, discord: 1})[0]?.id).toBe('code'));
  it('keeps matching quality ahead of launch frequency', () => expect(search(apps, 'visual studio', [], {code: 100})[0]?.id).toBe('studio'));
});
