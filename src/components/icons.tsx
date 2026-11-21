import type { ReactNode } from "react";

interface P { size?: number; className?: string }
const Svg = ({ size = 20, className, children, fill = true }: P & { children: ReactNode; fill?: boolean }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" className={className} aria-hidden
    fill={fill ? "currentColor" : "none"} stroke={fill ? "none" : "currentColor"} strokeWidth={fill ? 0 : 1.8}
    strokeLinecap="round" strokeLinejoin="round">
    {children}
  </svg>
);

export const PlayIcon = (p: P) => <Svg {...p}><path d="M8 5.5v13a1 1 0 001.5.86l10.5-6.5a1 1 0 000-1.72L9.5 4.64A1 1 0 008 5.5z" /></Svg>;
export const PauseIcon = (p: P) => <Svg {...p}><rect x="6.5" y="5" width="4" height="14" rx="1.2" /><rect x="13.5" y="5" width="4" height="14" rx="1.2" /></Svg>;
export const NextIcon = (p: P) => <Svg {...p}><path d="M5 6.2v11.6a.8.8 0 001.2.7l8.2-5.8a.8.8 0 000-1.4L6.2 5.5A.8.8 0 005 6.2z" /><rect x="16" y="5" width="2.6" height="14" rx="1.1" /></Svg>;
export const PrevIcon = (p: P) => <Svg {...p}><path d="M19 6.2v11.6a.8.8 0 01-1.2.7l-8.2-5.8a.8.8 0 010-1.4l8.2-5.8A.8.8 0 0119 6.2z" /><rect x="5.4" y="5" width="2.6" height="14" rx="1.1" /></Svg>;
export const ShuffleIcon = (p: P) => <Svg {...p} fill={false}><path d="M16 4h4v4M20 4l-6.5 6.5M4 20l16-16M16 20h4v-4M20 20l-5-5M4 4l5 5" /></Svg>;
export const RepeatIcon = (p: P) => <Svg {...p} fill={false}><path d="M17 2l3 3-3 3M4 11V9a4 4 0 014-4h12M7 22l-3-3 3-3M20 13v2a4 4 0 01-4 4H4" /></Svg>;
export const RepeatOneIcon = (p: P) => <Svg {...p} fill={false}><path d="M17 2l3 3-3 3M4 11V9a4 4 0 014-4h12M7 22l-3-3 3-3M20 13v2a4 4 0 01-4 4H4M11 10l1.5-1v6" /></Svg>;
export const HeartIcon = ({ filled, ...p }: P & { filled?: boolean }) => (
  <svg width={p.size ?? 18} height={p.size ?? 18} viewBox="0 0 24 24" className={p.className} aria-hidden
    fill={filled ? "currentColor" : "none"} stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round">
    <path d="M12 20.5s-7.5-4.6-9.2-9.4C1.6 7.6 3.7 4.5 7 4.5c2 0 3.6 1.1 5 3 1.4-1.9 3-3 5-3 3.3 0 5.4 3.1 4.2 6.6-1.7 4.8-9.2 9.4-9.2 9.4z" />
  </svg>
);
export const StarIcon = ({ filled, ...p }: P & { filled?: boolean }) => (
  <svg width={p.size ?? 14} height={p.size ?? 14} viewBox="0 0 24 24" className={p.className} aria-hidden
    fill={filled ? "currentColor" : "none"} stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round">
    <path d="M12 3.5l2.7 5.6 6.1.8-4.5 4.2 1.1 6-5.4-3-5.4 3 1.1-6L3.2 9.9l6.1-.8z" />
  </svg>
);
export const GearIcon = (p: P) => <Svg {...p} fill={false}><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 00.3 1.8l.1.1a2 2 0 11-2.8 2.8l-.1-.1a1.7 1.7 0 00-1.8-.3 1.7 1.7 0 00-1 1.5V21a2 2 0 11-4 0v-.1a1.7 1.7 0 00-1.1-1.5 1.7 1.7 0 00-1.8.3l-.1.1a2 2 0 11-2.8-2.8l.1-.1a1.7 1.7 0 00.3-1.8 1.7 1.7 0 00-1.5-1H3a2 2 0 110-4h.1a1.7 1.7 0 001.5-1.1 1.7 1.7 0 00-.3-1.8l-.1-.1a2 2 0 112.8-2.8l.1.1a1.7 1.7 0 001.8.3H9a1.7 1.7 0 001-1.5V3a2 2 0 114 0v.1a1.7 1.7 0 001 1.5 1.7 1.7 0 001.8-.3l.1-.1a2 2 0 112.8 2.8l-.1.1a1.7 1.7 0 00-.3 1.8V9a1.7 1.7 0 001.5 1H21a2 2 0 110 4h-.1a1.7 1.7 0 00-1.5 1z" /></Svg>;
export const SearchIcon = (p: P) => <Svg {...p} fill={false}><circle cx="11" cy="11" r="7" /><path d="M20 20l-3.5-3.5" /></Svg>;
export const CloseIcon = (p: P) => <Svg {...p} fill={false}><path d="M6 6l12 12M18 6L6 18" /></Svg>;
export const PencilIcon = (p: P) => <Svg {...p} fill={false}><path d="M4 20l1-4L16.5 4.5a2 2 0 013 3L8 19l-4 1z" /></Svg>;
export const PlusIcon = (p: P) => <Svg {...p} fill={false}><path d="M12 5v14M5 12h14" /></Svg>;
export const MinusIcon = (p: P) => <Svg {...p} fill={false}><path d="M5 12h14" /></Svg>;
export const BackIcon = (p: P) => <Svg {...p} fill={false}><path d="M15 5l-7 7 7 7" /></Svg>;
export const MiniIcon = (p: P) => <Svg {...p} fill={false}><rect x="3" y="5" width="18" height="14" rx="3" /><rect x="12" y="12" width="7" height="5" rx="1.5" fill="currentColor" /></Svg>;
export const ExpandIcon = (p: P) => <Svg {...p} fill={false}><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" /></Svg>;
export const TrashIcon = (p: P) => <Svg {...p} fill={false}><path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" /></Svg>;
export const FolderIcon = (p: P) => <Svg {...p} fill={false}><path d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" /></Svg>;
export const RefreshIcon = (p: P) => <Svg {...p} fill={false}><path d="M20 11a8 8 0 10-2.3 5.7M20 4v7h-7" /></Svg>;
export const ChevronIcon = (p: P) => <Svg {...p} fill={false}><path d="M6 9l6 6 6-6" /></Svg>;
export const VolumeIcon = ({ level = 1, ...p }: P & { level?: number }) => (
  <Svg {...p} fill={false}>
    <path d="M4 10v4h4l5 4V6L8 10z" fill="currentColor" />
    {level === 0 ? <path d="M17 9l5 6M22 9l-5 6" /> : <path d="M16.5 9a4 4 0 010 6" />}
    {level > 0.5 && <path d="M19 6.5a8 8 0 010 11" />}
  </Svg>
);

export const EyeIcon = ({ size = 24, ...props }: { size?: number } & React.SVGProps<SVGSVGElement>) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" {...props}>
    <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z" /><circle cx="12" cy="12" r="3" />
  </svg>
);

export const EyeOffIcon = ({ size = 24, ...props }: { size?: number } & React.SVGProps<SVGSVGElement>) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" {...props}>
    <path d="M9.88 9.88a3 3 0 1 0 4.24 4.24" /><path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68" /><path d="M6.61 6.61A13.526 13.526 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61" /><line x1="2" x2="22" y1="2" y2="22" />
  </svg>
);
