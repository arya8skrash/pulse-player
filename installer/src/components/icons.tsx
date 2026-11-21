interface P { size?: number; className?: string }
const base = (size: number, className?: string) => ({ width: size, height: size, viewBox: "0 0 24 24", className, "aria-hidden": true as const });

export const CheckIcon = ({ size = 16, className }: P) => (
  <svg {...base(size, className)} fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
);
export const FolderIcon = ({ size = 18, className }: P) => (
  <svg {...base(size, className)} fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" /></svg>
);
export const PlayIcon = ({ size = 20, className }: P) => (
  <svg {...base(size, className)} fill="currentColor"><path d="M8 5.5v13a1 1 0 001.5.86l10.5-6.5a1 1 0 000-1.72L9.5 4.64A1 1 0 008 5.5z" /></svg>
);
export const WarnIcon = ({ size = 16, className }: P) => (
  <svg {...base(size, className)} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M12 4l9 16H3z" /><path d="M12 10v4M12 17.5v.01" /></svg>
);
export const InfoIcon = ({ size = 16, className }: P) => (
  <svg {...base(size, className)} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="9" /><path d="M12 11v5M12 7.5v.01" /></svg>
);
export const CrossIcon = ({ size = 16, className }: P) => (
  <svg {...base(size, className)} fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
);
