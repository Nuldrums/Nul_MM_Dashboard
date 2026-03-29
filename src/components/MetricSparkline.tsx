import { LineChart, Line, ResponsiveContainer } from 'recharts';

interface MetricSparklineProps {
  data: { date: string; value: number }[];
  color?: string;
  width?: number;
  height?: number;
}

export default function MetricSparkline({
  data,
  color = 'var(--accent-primary)',
  width = 80,
  height = 32,
}: MetricSparklineProps) {
  if (!data || data.length < 2) {
    return <span style={{ width, height, display: 'inline-block' }} />;
  }

  return (
    <div style={{ width, height }}>
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={data}>
          <Line
            type="monotone"
            dataKey="value"
            stroke={color}
            strokeWidth={1.5}
            dot={false}
            isAnimationActive={false}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
