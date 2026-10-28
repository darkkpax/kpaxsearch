export interface Program { id: string; name: string; icon: string }

const latin = "qwertyuiop[]asdfghjkl;'zxcvbnm,.`";
const russian = "йцукенгшщзхъфывапролджэячсмитьбюё";
const normalize = (value: string) => value.toLocaleLowerCase().replace(/ё/g, 'е').trim();
function swapLayout(value: string): string {
  return [...value].map(c => {
    const ru = russian.indexOf(c), en = latin.indexOf(c);
    return ru >= 0 ? latin[ru] : en >= 0 ? russian[en] : c;
  }).join('');
}
function score(name: string, query: string): number {
  if (name === query) return 10000;
  if (name.startsWith(query)) return 8000 - name.length;
  if (name.split(/[\s.\-_]+/).some(word => word.startsWith(query))) return 7000 - name.length;
  if (name.includes(query)) return 6000 - name.length;
  const terms = query.split(/\s+/);
  if (terms.length > 1 && terms.every(term => name.includes(term))) return 5800 - name.length;
  const initials = name.split(/[\s.\-_]+/).map(word => word[0] ?? '').join('');
  if (initials.startsWith(query)) return 5500 - name.length;
  if (query.length < 2) return 0;
  let position = -1, gap = 0;
  for (const character of query) {
    const next = name.indexOf(character, position + 1);
    if (next < 0) return 0;
    gap += next - position - 1;
    position = next;
  }
  return 3000 - gap * 12 - name.length;
}

export function search(programs: Program[], input: string, recent: string[] = [], usage: Record<string, number> = {}): Program[] {
  const query = normalize(input), alternate = normalize(swapLayout(input.toLocaleLowerCase().trim()));
  const recency = new Map(recent.map((id, index) => [id, Math.max(0, 50 - index)]));
  return programs.map(program => ({
    program,
    score: query ? Math.max(score(normalize(program.name), query), score(normalize(program.name), alternate) * .95) : 1,
    recent: recency.get(program.id) ?? 0,
    frequency: Number.isFinite(usage[program.id]) ? Math.max(0, usage[program.id]) : 0,
  })).filter(result => result.score > 0)
    .sort((a, b) => b.score - a.score || b.frequency - a.frequency || b.recent - a.recent || a.program.name.localeCompare(b.program.name, 'ru'))
    .map(result => result.program);
}
