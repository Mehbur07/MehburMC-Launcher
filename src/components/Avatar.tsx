import { useTranslation } from "react-i18next";

import { SkinThumb } from "../features/skins/SkinThumb";

const DOT = {
  sm: "h-2 w-2 -right-0.5 -bottom-0.5",
  md: "h-3 w-3 -right-1 -bottom-1",
} as const;

/**
 * Profile photo of an account or friend: the chosen photo, else the head of
 * the skin, else a generic icon. `online` adds the neon lime dot at the
 * bottom-right.
 */
export function Avatar({
  photo,
  skin = null,
  unit = 5,
  className = "",
  online,
  dot = "md",
}: {
  /** `data:` URI of the profile photo. */
  photo: string | null | undefined;
  /** `data:` URI of the skin texture (head fallback). */
  skin?: string | null;
  /** Skin pixels → canvas pixels for the head fallback. */
  unit?: number;
  className?: string;
  online?: boolean;
  dot?: keyof typeof DOT;
}) {
  const { t } = useTranslation();
  const face = photo ? (
    <img src={photo} alt="" draggable={false} className={`object-cover ${className}`} />
  ) : (
    <SkinThumb src={skin} variant="head" unit={unit} className={className} />
  );
  if (!online) return face;
  return (
    <span className="relative flex shrink-0">
      {face}
      <span
        role="img"
        aria-label={t("friends.online")}
        title={t("friends.online")}
        className={`online-dot pointer-events-none absolute rounded-full ${DOT[dot]}`}
      />
    </span>
  );
}
