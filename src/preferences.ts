export interface Preferences { blur: boolean; material: 'acrylic' | 'blur' | 'mica'; radius: number; density: number; rows: number }
export const preferencesKey = 'kpax:appearance';
export function parsePreferences(raw: string | null): Preferences {
  let value: Record<string, unknown> = {};
  try { const parsed = JSON.parse(raw ?? '{}'); if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) value = parsed; } catch { /* use defaults */ }
  const number = (key: string, fallback: number, min: number, max: number) => typeof value[key] === 'number' && Number.isFinite(value[key]) ? Math.min(max, Math.max(min, Math.round(value[key]))) : fallback;
  return { blur: typeof value.blur === 'boolean' ? value.blur : true, material: value.material === 'blur' || value.material === 'mica' ? value.material : 'acrylic', radius: number('radius',16,0,40), density: number('density', 25, 0, 100), rows: number('rows', 7, 4, 10) };
}
