import type { SVGProps } from "react";

/**
 * MehburMC mark: an isometric cube outline whose front faces form an "M".
 * The M's inner "V" runs parallel to the cube's top edges and meets the
 * cube's front vertical edge. Original artwork.
 */
export function Logo(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 64 64" fill="none" aria-hidden="true" {...props}>
      <path
        d="M32 5 55 18v28L32 59 9 46V18Z"
        stroke="currentColor"
        strokeWidth="3"
        strokeLinejoin="round"
        opacity="0.55"
      />
      <path d="M32 33.5V59" stroke="currentColor" strokeWidth="2" opacity="0.35" />
      <path
        d="M17 44V25l15 8.5L47 25v19"
        stroke="currentColor"
        strokeWidth="5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
