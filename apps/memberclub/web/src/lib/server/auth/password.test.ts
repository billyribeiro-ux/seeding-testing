import { describe, it, expect } from 'vitest';
import { hashPassword, verifyPassword } from './password';

describe('password', () => {
  it('round-trips a valid password', () => {
    const h = hashPassword('correct horse battery staple');
    expect(verifyPassword('correct horse battery staple', h)).toBe(true);
  });

  it('rejects an incorrect password', () => {
    const h = hashPassword('correct horse battery staple');
    expect(verifyPassword('wrong password', h)).toBe(false);
  });

  it('produces different hashes for the same input (salted)', () => {
    const a = hashPassword('same password');
    const b = hashPassword('same password');
    expect(a).not.toBe(b);
    expect(verifyPassword('same password', a)).toBe(true);
    expect(verifyPassword('same password', b)).toBe(true);
  });

  it('rejects a malformed stored hash', () => {
    expect(verifyPassword('anything', 'not a real scrypt hash')).toBe(false);
  });

  it('PHC-style segments are preserved', () => {
    const h = hashPassword('x');
    const parts = h.split('$');
    expect(parts[0]).toBe('scrypt');
    expect(parts.length).toBe(6);
  });
});
