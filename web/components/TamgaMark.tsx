export function TamgaMark({ size = 96, className }: { size?: number; className?: string }) {
  return (
    <svg viewBox="0 0 64 64" width={size} height={size} className={className} role="img" aria-label="Tamga mark">
      <rect x="3" y="3" width="58" height="58" rx="14" fill="none" stroke="currentColor" strokeWidth="2" opacity=".55" />
      <path
        d="M32 12v40M32 30 19 17M32 30 45 17M21 44h22M24 52h16"
        fill="none"
        stroke="currentColor"
        strokeWidth="3.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx="32" cy="12" r="3" fill="currentColor" />
    </svg>
  );
}

export function Ornament({ id }: { id: string }) {
  return (
    <svg className="tg-ornament" width="100%" height="28" aria-hidden="true">
      <defs>
        <pattern id={id} width="56" height="28" patternUnits="userSpaceOnUse">
          <path
            d="M27 14C22 3 8 3 8 12c0 6 8 7 9 2 1-4-4-5-5-2"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
          />
          <path
            d="M27 14C22 3 8 3 8 12c0 6 8 7 9 2 1-4-4-5-5-2"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            transform="translate(56 0) scale(-1 1)"
          />
        </pattern>
      </defs>
      <rect width="100%" height="28" fill={`url(#${id})`} />
    </svg>
  );
}
