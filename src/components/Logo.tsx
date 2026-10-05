import logoUrl from "../assets/logo.png";

/** MehburMC Launcher logo (the "MMC" badge, same artwork as the app icon). */
export function Logo({ className = "" }: { className?: string }) {
  return <img src={logoUrl} alt="" aria-hidden="true" draggable={false} className={className} />;
}
