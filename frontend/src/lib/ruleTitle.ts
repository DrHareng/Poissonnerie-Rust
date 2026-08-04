/** Titre bleu, parenthèses finales en couleur normale — ex. « Prise (X) ». */
export function splitRuleTitle(name: string): {
  label: string
  suffix: string | null
} {
  const match = name.match(/^(.*?)(\s*\([^)]*\))\s*$/)
  if (!match?.[1]?.trim()) {
    return { label: name, suffix: null }
  }
  return { label: match[1].trimEnd(), suffix: match[2] ?? null }
}
