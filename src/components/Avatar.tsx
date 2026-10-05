import { SkinThumb } from "../features/skins/SkinThumb";

/**
 * Profile photo of an account or friend: the chosen photo, else the head of
 * the skin, else a generic icon.
 */
export function Avatar({
  photo,
  skin = null,
  unit = 5,
  className = "",
}: {
  /** `data:` URI of the profile photo. */
  photo: string | null | undefined;
  /** `data:` URI of the skin texture (head fallback). */
  skin?: string | null;
  /** Skin pixels → canvas pixels for the head fallback. */
  unit?: number;
  className?: string;
}) {
  if (photo) {
    return <img src={photo} alt="" draggable={false} className={`object-cover ${className}`} />;
  }
  return <SkinThumb src={skin} variant="head" unit={unit} className={className} />;
}
