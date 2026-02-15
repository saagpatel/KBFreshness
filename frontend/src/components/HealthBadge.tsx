import type { Article } from '../types';

interface HealthBadgeProps {
  health: Article['health'];
  size?: 'sm' | 'md' | 'lg';
}

export function HealthBadge({ health, size = 'md' }: HealthBadgeProps) {
  const colors = {
    green: 'bg-green-100 text-green-800',
    yellow: 'bg-yellow-100 text-yellow-800',
    red: 'bg-red-100 text-red-800',
  };

  const sizes = {
    sm: 'px-2 py-0.5 text-xs',
    md: 'px-2.5 py-1 text-sm',
    lg: 'px-3 py-1.5 text-base',
  };

  return (
    <span
      className={`inline-flex items-center rounded-full font-medium ${colors[health]} ${sizes[size]}`}
    >
      {health.charAt(0).toUpperCase() + health.slice(1)}
    </span>
  );
}
