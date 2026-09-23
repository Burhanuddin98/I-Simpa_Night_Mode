// The mockup's line icons (docs/design/concept-b-approved.dc.html), 20-unit grid.
const common = { viewBox: '0 0 20 20', fill: 'none', 'aria-hidden': true } as const;

export const Search = ({ size = 13 }: { size?: number }) => (
  <svg width={size} height={size} {...common}>
    <circle cx="9" cy="9" r="5.5" stroke="currentColor" strokeWidth="1.8" />
    <path d="M13.2 13.2L17 17" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
  </svg>
);

export const Play = () => (
  <svg width="11" height="11" viewBox="0 0 20 20" aria-hidden>
    <path d="M5 3l12 7-12 7z" fill="currentColor" />
  </svg>
);

export const SelectTool = () => (
  <svg width="16" height="16" {...common}>
    <path d="M5 3l10 6-4.6 1.4L8 15.5z" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" />
  </svg>
);

export const OrbitTool = () => (
  <svg width="16" height="16" {...common}>
    <ellipse cx="10" cy="10" rx="7.5" ry="3.2" stroke="currentColor" strokeWidth="1.5" />
    <circle cx="10" cy="10" r="1.8" fill="currentColor" />
  </svg>
);

export const ReceiverTool = () => (
  <svg width="16" height="16" {...common}>
    <circle cx="10" cy="7" r="3" stroke="currentColor" strokeWidth="1.5" />
    <path d="M10 10v7M7 17h6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

export const SectionTool = () => (
  <svg width="16" height="16" {...common}>
    <path d="M3 12l7-4 7 4-7 4z" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
    <path d="M10 3v5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

export const MeasureTool = () => (
  <svg width="16" height="16" {...common}>
    <path d="M3 13.5L13.5 3 17 6.5 6.5 17z" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
    <path d="M6.5 10l1.8 1.8M9.3 7.2l1.8 1.8M12.1 4.4l1.8 1.8" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
  </svg>
);

/** The axis gizmo: x red, y grey, z white. */
export const AxisGizmo = () => (
  <svg className="gizmo" width="60" height="60" viewBox="0 0 64 64" aria-hidden>
    <path d="M32 36l16-6.6" stroke="#E0202E" strokeWidth="1.6" strokeLinecap="round" />
    <path d="M32 36l12.3 8.4" stroke="#A1A1AA" strokeWidth="1.6" strokeLinecap="round" />
    <path d="M32 36V19" stroke="#EDEDEF" strokeWidth="1.6" strokeLinecap="round" />
    <text x="51" y="30" fontSize="9" fill="#E0202E" fontFamily="JetBrains Mono, monospace">
      x
    </text>
    <text x="46" y="52" fontSize="9" fill="#A1A1AA" fontFamily="JetBrains Mono, monospace">
      y
    </text>
    <text x="29" y="15" fontSize="9" fill="#EDEDEF" fontFamily="JetBrains Mono, monospace">
      z
    </text>
  </svg>
);
