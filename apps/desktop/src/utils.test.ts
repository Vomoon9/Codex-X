import { describe, it, expect } from 'vitest';
import {
  cx,
  providerId,
  tomlEscape,
  extractOpenAiApiKey,
  buildProviderTomlPreview,
  buildProviderAuthPreview,
  instructionIdFromPath,
  normalizeVersion,
  compareVersions,
  releaseAssetForPlatform,
  formatSessionTime,
  compactPath,
  shortId,
} from './utils';

describe('cx', () => {
  it('joins truthy class names with spaces', () => {
    expect(cx('btn', true && 'active', false, undefined, 'primary')).toBe('btn active primary');
  });

  it('returns an empty string when all values are falsy', () => {
    expect(cx(false, undefined, null, '')).toBe('');
  });
});

describe('providerId', () => {
  it('slugifies names to lowercase and dashes', () => {
    expect(providerId('Hello World')).toBe('hello-world');
    expect(providerId('My--Provider!!!')).toBe('my-provider');
  });

  it('falls back to a provider- prefixed timestamp when sanitization leaves nothing', () => {
    expect(providerId('   ')).toMatch(/^provider-[0-9]+$/);
    expect(providerId('!!!')).toMatch(/^provider-[0-9]+$/);
  });
});

describe('tomlEscape', () => {
  it('escapes backslashes and double quotes', () => {
    const q = String.fromCharCode(34);
    const s = String.fromCharCode(92);
    const input = 'a' + q + 'b' + s + 'c';
    const expected = 'a' + s + q + 'b' + s + s + 'c';
    expect(tomlEscape(input)).toBe(expected);
  });
});

describe('extractOpenAiApiKey', () => {
  it('returns the OPENAI_API_KEY string when present', () => {
    const valid = JSON.stringify({ OPENAI_API_KEY: 'sk-123' });
    expect(extractOpenAiApiKey(valid)).toBe('sk-123');
  });

  it('returns an empty string when the key is missing or not a string', () => {
    const numeric = JSON.stringify({ OPENAI_API_KEY: 123 });
    expect(extractOpenAiApiKey(numeric)).toBe('');
    expect(extractOpenAiApiKey(JSON.stringify({ other: 'x' }))).toBe('');
    expect(extractOpenAiApiKey('')).toBe('');
  });

  it('returns an empty string for invalid JSON', () => {
    expect(extractOpenAiApiKey('not json')).toBe('');
  });
});

describe('buildProviderTomlPreview', () => {
  const baseProvider = {
    id: 'magicai',
    providerName: 'MagicAI',
    baseUrl: 'https://sky1818.com',
    model: 'gpt-5.5',
    apiKey: '',
    wireApi: 'responses',
    requiresOpenaiAuth: true,
  };

  it('builds a fresh provider TOML when no existing config is provided', () => {
    const q = String.fromCharCode(34);
    const out = buildProviderTomlPreview(baseProvider, null);
    expect(out).toContain('model_provider = ' + q + 'custom' + q);
    expect(out).toContain('model = ' + q + 'gpt-5.5' + q);
    expect(out).toContain('model_reasoning_effort = ' + q + 'high' + q);
    expect(out).toContain('[model_providers.custom]');
    expect(out).toContain('name = ' + q + 'MagicAI' + q);
    expect(out).toContain('base_url = ' + q + 'https://sky1818.com' + q);
    expect(out).toContain('wire_api = ' + q + 'responses' + q);
    expect(out).toContain('requires_openai_auth = true');
  });

  it('strips trailing slashes from base URL', () => {
    const q = String.fromCharCode(34);
    const provider = { ...baseProvider, baseUrl: 'https://sky1818.com///' };
    expect(buildProviderTomlPreview(provider, null)).toContain('base_url = ' + q + 'https://sky1818.com' + q);
  });

  it('uses defaults for empty provider fields', () => {
    const q = String.fromCharCode(34);
    const provider = {
      ...baseProvider,
      providerName: '',
      baseUrl: '',
      model: '',
      wireApi: '',
      requiresOpenaiAuth: false,
    };
    const out = buildProviderTomlPreview(provider, null);
    expect(out).toContain('model = ' + q + 'gpt-5.5' + q);
    expect(out).toContain('name = ' + q + 'your-provider' + q);
    expect(out).toContain('base_url = ' + q + 'https://example.com/v1' + q);
    expect(out).toContain('wire_api = ' + q + 'responses' + q);
    expect(out).toContain('requires_openai_auth = false');
  });

  it('preserves non-provider root keys and other sections while replacing the custom provider', () => {
    const q = String.fromCharCode(34);
    const nl = String.fromCharCode(10);
    const state = {
      configText: [
        'model_provider = ' + q + 'openai' + q,
        'model = ' + q + 'gpt-5.4' + q,
        'foo = ' + q + 'bar' + q,
        '',
        '[model_providers.other]',
        'name = ' + q + 'Other' + q,
        'base_url = ' + q + 'https://other.com' + q,
        '',
        '[model_providers.custom]',
        'name = ' + q + 'Old' + q,
        'base_url = ' + q + 'https://old.com' + q,
      ].join(nl),
    };
    const out = buildProviderTomlPreview(baseProvider, state);
    expect(out).toContain('model = ' + q + 'gpt-5.5' + q);
    expect(out).toContain('model_provider = ' + q + 'custom' + q);
    expect(out).toContain('foo = ' + q + 'bar' + q);
    expect(out).toContain('[model_providers.other]');
    expect(out).not.toContain('name = ' + q + 'Old' + q);
    expect(out).not.toContain('https://old.com');
    expect(out).toContain('name = ' + q + 'MagicAI' + q);
    expect(out).toContain('base_url = ' + q + 'https://sky1818.com' + q);
  });

  it('does not add default model_reasoning_effort when one is already present', () => {
    const q = String.fromCharCode(34);
    const state = { configText: 'model_reasoning_effort = ' + q + 'medium' + q };
    const out = buildProviderTomlPreview(baseProvider, state);
    expect(out).not.toContain('model_reasoning_effort = ' + q + 'high' + q);
    expect(out).toContain('[model_providers.custom]');
  });
});

describe('buildProviderAuthPreview', () => {
  it('returns a JSON object with the trimmed API key', () => {
    const out = buildProviderAuthPreview({
      id: 'x',
      providerName: 'x',
      baseUrl: 'x',
      model: 'x',
      apiKey: '  sk-123  ',
      wireApi: 'responses',
      requiresOpenaiAuth: true,
    });
    expect(JSON.parse(out).OPENAI_API_KEY).toBe('sk-123');
  });

  it('returns null for a missing or whitespace-only API key', () => {
    const out = buildProviderAuthPreview({
      id: 'x',
      providerName: 'x',
      baseUrl: 'x',
      model: 'x',
      wireApi: 'responses',
      requiresOpenaiAuth: true,
    });
    expect(JSON.parse(out).OPENAI_API_KEY).toBeNull();
  });
});

describe('instructionIdFromPath', () => {
  const templates = [
    { id: 'gpt5.5-unrestricted', filename: 'gpt5.5-unrestricted.md' },
    { id: 'gpt5.4-unrestricted', filename: 'gpt5.4-unrestricted.md' },
  ];

  it('matches known instruction filenames from any parent path', () => {
    const b = String.fromCharCode(92);
    const winPath = 'C:' + b + 'Users' + b + 'x' + b + '.codex' + b + 'gpt5.5-unrestricted.md';
    expect(instructionIdFromPath(winPath, templates)).toBe('gpt5.5-unrestricted');
    expect(instructionIdFromPath('/home/x/.codex/gpt5.4-unrestricted.md', templates)).toBe('gpt5.4-unrestricted');
  });

  it('returns custom for unknown or empty paths', () => {
    expect(instructionIdFromPath('/tmp/other.md', templates)).toBe('custom');
    expect(instructionIdFromPath(null, templates)).toBe('');
    expect(instructionIdFromPath('', templates)).toBe('');
  });
});

describe('normalizeVersion', () => {
  it('trims whitespace and removes a leading v', () => {
    expect(normalizeVersion('  v0.2.17  ')).toBe('0.2.17');
    expect(normalizeVersion('V1.0.0')).toBe('1.0.0');
  });

  it('returns an empty string for missing input', () => {
    expect(normalizeVersion(undefined)).toBe('');
    expect(normalizeVersion(null)).toBe('');
  });
});

describe('compareVersions', () => {
  it('compares versions numerically', () => {
    expect(compareVersions('0.2.17', '0.2.16')).toBe(1);
    expect(compareVersions('0.2.16', '0.2.17')).toBe(-1);
    expect(compareVersions('0.2.17', '0.2.17')).toBe(0);
    expect(compareVersions('1.0.0', '0.9.9')).toBe(1);
  });

  it('handles v-prefixes and different segment lengths', () => {
    expect(compareVersions('v0.2.0', '0.2')).toBe(0);
    expect(compareVersions('0.2.17', '0.2.17.1')).toBe(-1);
  });
});

describe('releaseAssetForPlatform', () => {
  const assets = [
    { name: 'Codex-X.exe' },
    { name: 'Codex-X.dmg' },
    { name: 'Codex-X.deb' },
  ];

  it('selects the right asset for each platform', () => {
    expect(releaseAssetForPlatform(assets, 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)')).toEqual({ name: 'Codex-X.exe' });
    expect(releaseAssetForPlatform(assets, 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)')).toEqual({ name: 'Codex-X.dmg' });
    expect(releaseAssetForPlatform(assets, 'Mozilla/5.0 (X11; Linux x86_64)')).toEqual({ name: 'Codex-X.deb' });
  });

  it('falls back to the first asset when the platform is unrecognized', () => {
    expect(releaseAssetForPlatform(assets, 'SomeBot/1.0')).toEqual({ name: 'Codex-X.exe' });
  });

  it('returns undefined when the asset list is empty', () => {
    expect(releaseAssetForPlatform([], 'Windows')).toBeUndefined();
  });
});

describe('formatSessionTime', () => {
  const unknownTime = String.fromCharCode(0x672a, 0x77e5, 0x65f6, 0x95f4);

  it('formats valid millisecond timestamps', () => {
    const out = formatSessionTime(1705312200000);
    expect(out).not.toBe(unknownTime);
    expect(out).toMatch(/[0-9]{1,2}:[0-9]{2}/);
  });

  it('returns unknown-time for missing or invalid values', () => {
    expect(formatSessionTime(null)).toBe(unknownTime);
    expect(formatSessionTime(0)).toBe(unknownTime);
    expect(formatSessionTime(8640000000000001)).toBe(unknownTime);
  });
});

describe('compactPath', () => {
  const unknownPath = String.fromCharCode(0x672a, 0x8bb0, 0x5f55, 0x8def, 0x5f84);
  const ellipsis = String.fromCharCode(0x2026);

  it('normalizes backslashes and leaves short paths unchanged', () => {
    const b = String.fromCharCode(92);
    const winShort = 'C:' + b + 'Users' + b + 'x' + b + '.codex';
    expect(compactPath(winShort)).toBe('C:/Users/x/.codex');
  });

  it('returns unknown-path for missing values', () => {
    expect(compactPath(null)).toBe(unknownPath);
    expect(compactPath('')).toBe(unknownPath);
  });

  it('shortens long paths with at least three parts using an ellipsis and tail', () => {
    const long = '/a/very/long/directory/path/that/is/too/long/to/show/again/more';
    const out = compactPath(long);
    expect(out).toBe(ellipsis + '/show/again/more');
  });

  it('shortens long paths with fewer than three parts using a suffix ellipsis', () => {
    const long = 'C:/' + 'a'.repeat(90);
    const out = compactPath(long);
    expect(out.startsWith(ellipsis)).toBe(true);
    expect(out.length).toBeLessThanOrEqual(58);
  });
});

describe('shortId', () => {
  it('truncates long IDs to 8 characters', () => {
    expect(shortId('1234567890')).toBe('12345678');
  });

  it('leaves short IDs unchanged', () => {
    expect(shortId('123')).toBe('123');
    expect(shortId('')).toBe('');
  });
});
