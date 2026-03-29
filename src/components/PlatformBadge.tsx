import { PLATFORM_COLORS, PLATFORM_NAMES } from '../lib/constants';

interface PlatformBadgeProps {
  platform: string;
  showName?: boolean;
}

export default function PlatformBadge({ platform, showName = true }: PlatformBadgeProps) {
  const color = PLATFORM_COLORS[platform] ?? PLATFORM_COLORS.other;
  const name = PLATFORM_NAMES[platform] ?? platform;

  return (
    <span className="flex-gap" style={{ gap: 6 }}>
      <span
        style={{
          backgroundColor: color,
          width: 10,
          height: 10,
          borderRadius: '50%',
          display: 'inline-block',
          flexShrink: 0,
        }}
      />
      {showName && (
        <span style={{ fontSize: '0.825rem', fontWeight: 500 }}>{name}</span>
      )}
    </span>
  );
}
