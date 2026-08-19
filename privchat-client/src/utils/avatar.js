// Telegram-style deterministic avatar colors from a name/peer string.
const PALETTE = [
  "#e17076",
  "#efa873",
  "#7bc862",
  "#54a0eb",
  "#5bc8ed",
  "#7787eb",
  "#cb6ee0",
  "#ee6c98",
];

export function avatarColor(seed) {
  let hash = 0;
  for (let i = 0; i < seed.length; i++) {
    hash = (hash * 31 + seed.charCodeAt(i)) >>> 0;
  }
  return PALETTE[hash % PALETTE.length];
}

export function initials(name) {
  const clean = name.trim();
  if (!clean) return "?";
  const parts = clean.split(/\s+/);
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
}
