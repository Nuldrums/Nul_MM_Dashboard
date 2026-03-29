import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';

interface EngagementChartProps {
  data: { date: string; value: number; [key: string]: any }[];
  title?: string;
  dataKeys?: { key: string; color: string; name: string }[];
}

export default function EngagementChart({
  data,
  title,
  dataKeys,
}: EngagementChartProps) {
  const keys = dataKeys ?? [
    { key: 'value', color: 'var(--chart-1)', name: 'Engagement' },
  ];

  return (
    <div className="card">
      {title && (
        <h3
          style={{ fontSize: '0.95rem', fontWeight: 600, marginBottom: 16 }}
        >
          {title}
        </h3>
      )}
      <div style={{ width: '100%', height: 280 }}>
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart
            data={data}
            margin={{ top: 4, right: 4, left: -12, bottom: 0 }}
          >
            <CartesianGrid
              strokeDasharray="3 3"
              stroke="var(--border-light)"
            />
            <XAxis
              dataKey="date"
              tick={{ fill: 'var(--text-tertiary)', fontSize: 11 }}
              tickLine={false}
              axisLine={{ stroke: 'var(--border-light)' }}
            />
            <YAxis
              tick={{ fill: 'var(--text-tertiary)', fontSize: 11 }}
              tickLine={false}
              axisLine={false}
            />
            <Tooltip
              contentStyle={{
                background: 'var(--surface-card)',
                border: '1px solid var(--border-medium)',
                borderRadius: 'var(--radius-sm)',
                fontSize: '0.8rem',
              }}
            />
            {keys.map((k) => (
              <Area
                key={k.key}
                type="monotone"
                dataKey={k.key}
                name={k.name}
                stroke={k.color}
                fill={k.color}
                fillOpacity={0.1}
                strokeWidth={2}
              />
            ))}
          </AreaChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
