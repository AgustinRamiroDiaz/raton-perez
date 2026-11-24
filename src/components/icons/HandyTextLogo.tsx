const APP_NAME = "🐭 Ratón Pérez";

// Text wordmark in the same 930x328 box the original logo used, so callers'
// width-only sizing keeps the same proportions.
const HandyTextLogo = ({
  width,
  height,
  className,
}: {
  width?: number;
  height?: number;
  className?: string;
}) => {
  return (
    <svg
      width={width}
      height={height}
      className={className}
      viewBox="0 0 930 328"
      xmlns="http://www.w3.org/2000/svg"
      role="img"
      aria-label={APP_NAME}
    >
      <text
        x="465"
        y="210"
        textAnchor="middle"
        fontSize="150"
        fontWeight="700"
        className="fill-text"
      >
        {APP_NAME}
      </text>
    </svg>
  );
};

export default HandyTextLogo;
