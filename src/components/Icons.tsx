import type { ReactNode } from 'react';

interface IconProps {
  size?: number;
  className?: string;
  strokeWidth?: number;
}

function svg(paths: ReactNode, filled = false) {
  return function Icon({ size = 18, className, strokeWidth = 1.8 }: IconProps) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill={filled ? 'currentColor' : 'none'}
        stroke={filled ? 'none' : 'currentColor'}
        strokeWidth={strokeWidth}
        strokeLinecap="round"
        strokeLinejoin="round"
        className={className}
        aria-hidden="true"
      >
        {paths}
      </svg>
    );
  };
}

export const IconChat = svg(
  <path d="M20 11.5a8.5 8.5 0 0 1-8.5 8.5H5l-2 2V11.5a8.5 8.5 0 0 1 8.5-8.5h.5a8 8 0 0 1 8 8v.5Z" />,
);
export const IconSend = svg(
  <>
    <path d="M21 3 10.5 13.5" />
    <path d="M21 3l-6.5 18-4-8-8-4L21 3Z" />
  </>,
);
export const IconPaperclip = svg(
  <path d="M20 11.5 12 19.5a5 5 0 0 1-7-7l8.5-8.5a3.5 3.5 0 0 1 5 5L10 17.5a2 2 0 0 1-3-3L15 7" />,
);
export const IconSearch = svg(
  <>
    <circle cx="11" cy="11" r="7" />
    <path d="m20 20-3.2-3.2" />
  </>,
);
export const IconSettings = svg(
  <>
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z" />
  </>,
);
export const IconUsers = svg(
  <>
    <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
    <circle cx="9" cy="7" r="4" />
    <path d="M22 21v-2a4 4 0 0 0-3-3.9" />
    <path d="M16 3.1a4 4 0 0 1 0 7.8" />
  </>,
);
export const IconHash = svg(
  <>
    <path d="M4 9h16M4 15h16M10 3 8 21M16 3l-2 18" />
  </>,
);
export const IconGlobe = svg(
  <>
    <circle cx="12" cy="12" r="9" />
    <path d="M3.5 9h17M3.5 15h17" />
    <path d="M12 3a15 15 0 0 1 0 18 15 15 0 0 1 0-18Z" />
  </>,
);
export const IconBell = svg(
  <>
    <path d="M18 8a6 6 0 0 0-12 0c0 7-3 9-3 9h18s-3-2-3-9" />
    <path d="M13.7 21a2 2 0 0 1-3.4 0" />
  </>,
);
export const IconBellOff = svg(
  <>
    <path d="M13.7 21a2 2 0 0 1-3.4 0" />
    <path d="M18.6 13A9 9 0 0 0 18 8a6 6 0 0 0-9.3-5" />
    <path d="M6 6a6 6 0 0 0 0 2c0 7-3 9-3 9h14" />
    <path d="m2 2 20 20" />
  </>,
);
export const IconX = svg(<path d="M18 6 6 18M6 6l12 12" />);
export const IconChevronDown = svg(<path d="m6 9 6 6 6-6" />);
export const IconChevronLeft = svg(<path d="m15 18-6-6 6-6" />);
export const IconChevronRight = svg(<path d="m9 18 6-6-6-6" />);
export const IconCheck = svg(<path d="m4 12 5 5L20 6" />);
export const IconCheckDouble = svg(
  <>
    <path d="m2 12 5 5L16 8" />
    <path d="m12 15 2 2L23 8" />
  </>,
);
export const IconImage = svg(
  <>
    <rect x="3" y="3" width="18" height="18" rx="3" />
    <circle cx="8.5" cy="8.5" r="1.5" />
    <path d="m21 15-5-5L5 21" />
  </>,
);
export const IconVideo = svg(
  <>
    <rect x="2" y="6" width="14" height="12" rx="2.5" />
    <path d="m16 10 6-3v10l-6-3" />
  </>,
);
export const IconFile = svg(
  <>
    <path d="M14 2H7a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7l-5-5Z" />
    <path d="M14 2v5h5" />
  </>,
);
export const IconDownload = svg(
  <>
    <path d="M12 3v12" />
    <path d="m7 10 5 5 5-5" />
    <path d="M4 21h16" />
  </>,
);
export const IconExternal = svg(
  <>
    <path d="M14 3h7v7" />
    <path d="M10 14 21 3" />
    <path d="M21 14v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5" />
  </>,
);
export const IconPlus = svg(<path d="M12 5v14M5 12h14" />);
export const IconMoon = svg(<path d="M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8Z" />);
export const IconSun = svg(
  <>
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
  </>,
);
export const IconSmile = svg(
  <>
    <circle cx="12" cy="12" r="9" />
    <path d="M8.5 14.5a4.5 4.5 0 0 0 7 0" />
    <path d="M9 9.5h.01M15 9.5h.01" />
  </>,
);
export const IconWifi = svg(
  <>
    <path d="M2.5 8.5a15 15 0 0 1 19 0" />
    <path d="M5.5 12a10.5 10.5 0 0 1 13 0" />
    <path d="M8.5 15.5a6 6 0 0 1 7 0" />
    <path d="M12 19h.01" />
  </>,
);
export const IconMic = svg(
  <>
    <rect x="9" y="2" width="6" height="11" rx="3" />
    <path d="M5 11a7 7 0 0 0 14 0" />
    <path d="M12 18v4M8 22h8" />
  </>,
);
export const IconTrash = svg(
  <>
    <path d="M3 6h18" />
    <path d="M8 6V4h8v2" />
    <path d="m5 6 1 15h12l1-15" />
    <path d="M10 11v6M14 11v6" />
  </>,
);
export const IconLogOut = svg(
  <>
    <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
    <path d="m16 17 5-5-5-5" />
    <path d="M21 12H9" />
  </>,
);
export const IconMinimize = svg(<path d="M6 12h12" />);
export const IconEdit = svg(
  <>
    <path d="M12 20h9" />
    <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5Z" />
  </>,
);
export const IconMore = svg(
  <>
    <circle cx="5" cy="12" r="1.2" />
    <circle cx="12" cy="12" r="1.2" />
    <circle cx="19" cy="12" r="1.2" />
  </>,
);
export const IconLock = svg(
  <>
    <rect x="4" y="10" width="16" height="11" rx="2.5" />
    <path d="M8 10V7a4 4 0 0 1 8 0v3" />
  </>,
);
