/** The app's P, the same shape as the app icon, so it sits dead centre in its plate whatever the theme's font. */
export function Logo() {
  return (
    <span className="logo">
      <svg viewBox="0 0 124 140" aria-hidden="true">
        <path d="M0 0H96L124 28V70L96 98H38V140H0Z M38 34V64H84L92 56V42L84 34Z" fillRule="evenodd" />
      </svg>
    </span>
  );
}
