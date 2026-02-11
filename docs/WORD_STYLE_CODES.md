# Word-Style Code Format

Used for device registration codes (webapp → executor). Human-readable, easy to type on mobile, avoids ambiguous characters.

---

## 1. Device Registration Code (Webapp)

**Purpose:** Code displayed by webapp keygen, typed into executor CLI for `register-device`.

### Format

- **Pattern:** `word1-word2-word3-word4` (4 words, hyphen-separated)
- **Case:** lowercase
- **Word list:** EFF short list (1296 words, 4–5 chars)
- **Entropy:** ~42 bits (log₂(1296⁴))
- **Example:** `echo-brick-zeta-quip`

### Rules

- Use cryptographically secure random selection (`crypto.getRandomValues`)
- Words drawn from [EFF Short Word List 1](https://www.eff.org/dice) or equivalent
- No ambiguous pairs (e.g. avoid 0/O, 1/l; EFF list chosen to minimize these)
- Max length: ~25 chars
- Valid regex: `^[a-z]+-[a-z]+-[a-z]+-[a-z]+$`

### Webapp Generation (Pseudocode)

```ts
const WORD_LIST = [/* EFF short 1296 words */];

function generateRegistrationCode(): string {
  const words = Array.from(
    { length: 4 },
    () => WORD_LIST[crypto.getRandomValues(new Uint32Array(1))[0] % WORD_LIST.length]
  );
  return words.join("-");
}
```

### Storage (Relayer)

- Store as plain text in `device_registration_codes.code`
- Case-sensitive match on `register-device`
- Expire after `code_ttl_secs` (default 600)
- Single-use: set `used = 1` on first successful registration

---

## 2. Executor API Key (.env)

**Purpose:** Pre-shared secret between relayer and executor. Not word-style—hex for `.env` safety.

### Format

- **Pattern:** 64 hex characters (32 bytes)
- **Generation:** `openssl rand -hex 32`
- **Example:** `a1b2c3d4e5f6789012345678abcdef0123456789abcdef0123456789abcd`

### Why Hex (Not Word-Style)

- Safe for `.env` files (no quotes, no special chars, no spaces)
- No line-break issues
- Standard tooling (`openssl`)
- Copy-paste friendly between relayer and executor configs

### Validation

- Length: exactly 64 chars
- Regex: `^[0-9a-f]{64}$`
