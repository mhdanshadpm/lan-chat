import { useState, type FormEvent } from 'react';
import { Avatar } from './Avatar';
import { BrandMark } from './Brand';
import { IconLock } from './Icons';
import { cls } from '../utils';

const PALETTE = [
  0xe06c75, 0xf29e6e, 0xf5d06b, 0x8bd49a, 0x6bd3c9,
  0x74aaf7, 0x9b8cf5, 0xe08cf5, 0xf58cc0, 0x9aa6b4,
];

interface Props {
  busy: boolean;
  error: string | null;
  onDone: (name: string, color: number, passphrase: string | null) => void;
}

export function FirstRun({ busy, error, onDone }: Props) {
  const [name, setName] = useState('');
  const [color, setColor] = useState(PALETTE[5]);
  const [passphrase, setPassphrase] = useState('');
  const [showPassphrase, setShowPassphrase] = useState(false);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!name.trim() || busy) return;
    onDone(name.trim(), color, passphrase.trim() || null);
  };

  return (
    <div className="flex min-h-full items-center justify-center px-4 py-10">
      <div className="w-full max-w-md">
        <div className="mb-7 flex flex-col items-center text-center">
          <BrandMark size={56} />
          <h1 className="mt-4 text-2xl font-semibold tracking-tight">Welcome to LAN Chat</h1>
          <p className="mt-2 max-w-sm text-sm leading-relaxed" style={{ color: 'var(--muted)' }}>
            Messages travel directly between devices on your Wi-Fi or wired network. No accounts,
            no cloud, no internet required.
          </p>
        </div>

        <form onSubmit={submit} className="card animate-rise p-6 shadow-soft">
          <div className="flex items-center gap-4">
            <Avatar name={name || 'You'} color={color} size={58} />
            <div className="min-w-0 flex-1">
              <label className="label" htmlFor="display-name">
                Display name
              </label>
              <input
                id="display-name"
                className="field"
                placeholder="How should people see you?"
                value={name}
                maxLength={64}
                autoFocus
                onChange={(event) => setName(event.target.value)}
              />
            </div>
          </div>

          <div className="mt-5">
            <span className="label">Avatar colour</span>
            <div className="flex flex-wrap gap-2">
              {PALETTE.map((value) => (
                <button
                  key={value}
                  type="button"
                  aria-label={`Choose colour ${value.toString(16)}`}
                  onClick={() => setColor(value)}
                  className={cls(
                    'h-8 w-8 rounded-full transition-transform hover:scale-110',
                    color === value && 'ring-2 ring-offset-2',
                  )}
                  style={{
                    background: `#${value.toString(16).padStart(6, '0')}`,
                    boxShadow: color === value ? '0 0 0 3px var(--accent-soft)' : undefined,
                  }}
                />
              ))}
            </div>
          </div>

          <div className="mt-6 rounded-xl border p-3.5 divider" style={{ background: 'var(--bg-soft)' }}>
            <div className="flex items-center justify-between gap-3">
              <span className="flex items-center gap-2 text-sm font-medium">
                <IconLock size={16} />
                LAN passphrase
                <span className="chip">optional</span>
              </span>
              <button
                type="button"
                className="text-xs font-semibold"
                style={{ color: 'var(--accent)' }}
                onClick={() => setShowPassphrase((value) => !value)}
              >
                {showPassphrase ? 'Hide' : 'Add'}
              </button>
            </div>
            {showPassphrase && (
              <div className="mt-3">
                <input
                  type="password"
                  className="field"
                  placeholder="Shared secret for this network"
                  value={passphrase}
                  onChange={(event) => setPassphrase(event.target.value)}
                />
                <p className="mt-2 text-xs leading-relaxed" style={{ color: 'var(--muted)' }}>
                  Everyone on the LAN must use the same passphrase. Frames are encrypted with
                  XChaCha20-Poly1305 and the key is derived with Argon2id.
                </p>
              </div>
            )}
          </div>

          {error && <p className="mt-4 text-sm text-red-500">{error}</p>}

          <button type="submit" className="btn-primary mt-6 w-full py-2.5" disabled={!name.trim() || busy}>
            {busy ? 'Starting…' : 'Start chatting'}
          </button>
        </form>

        <p className="mt-5 text-center text-xs leading-relaxed" style={{ color: 'var(--muted)' }}>
          On macOS, allow the local network prompt so peers can be discovered. Windows may ask
          once about firewall access — allow private networks.
        </p>
      </div>
    </div>
  );
}
