const nameMax = 80;

export function normalizeName(value: string): string {
  const name = value.trim();
  return name.endsWith('.txt') ? name.slice(0, -4) : name;
}

// Accepts empty (the save request then omits the name header and the server
// assigns a timestamp id) or a basename of letters, digits, dots, dashes, and
// underscores with no leading or trailing dot. The nonempty rules mirror
// valid_name in src/gist.rs, which itself rejects empty names.
export function validName(name: string): boolean {
  return (
    name.length <= nameMax &&
    (name === '' ||
      (name !== '.' &&
        name !== '..' &&
        !name.startsWith('.') &&
        !name.endsWith('.') &&
        /^[A-Za-z0-9._-]+$/.test(name)))
  );
}
