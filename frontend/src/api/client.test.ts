import { describe, expect, it } from 'vitest';
import { normalizeApiBase } from './client';

describe('normalizeApiBase', () => {
  it('returns default API path when env is missing', () => {
    expect(normalizeApiBase(undefined, 'http://localhost:5173')).toBe('/api');
  });

  it('keeps absolute API URLs unchanged', () => {
    expect(normalizeApiBase('https://api.example.com', 'http://localhost:5173')).toBe(
      'https://api.example.com',
    );
  });

  it('normalizes relative paths against origin', () => {
    expect(normalizeApiBase('/backend/api', 'http://localhost:5173')).toBe('/backend/api');
  });

  it('throws on invalid URL strings', () => {
    expect(() => normalizeApiBase('http:// bad url', 'http://localhost:5173')).toThrow(
      /Invalid VITE_API_URL format/,
    );
  });
});
