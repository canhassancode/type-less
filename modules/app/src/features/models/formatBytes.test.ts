import { describe, expect, test } from 'vitest';
import { formatBytes } from './formatBytes';

describe('formatBytes', () => {
  test('renders bytes for small values', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(512)).toBe('512 B');
  });

  test('scales to kilobytes, megabytes, gigabytes', () => {
    expect(formatBytes(1024)).toBe('1.0 KB');
    expect(formatBytes(1024 * 1024)).toBe('1.0 MB');
    expect(formatBytes(1024 * 1024 * 1024)).toBe('1.0 GB');
  });

  test('uses no decimals when value >= 100 in its unit', () => {
    expect(formatBytes(123 * 1024 * 1024)).toBe('123 MB');
  });
});
