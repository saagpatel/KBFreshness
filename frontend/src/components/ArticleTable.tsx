import { useState } from 'react';
import { useArticles } from '../api/client';
import { HealthBadge } from './HealthBadge';

interface ArticleTableProps {
  onArticleClick: (id: string) => void;
}

export function ArticleTable({ onArticleClick }: ArticleTableProps) {
  const [healthFilter, setHealthFilter] = useState<string | undefined>();
  const [sortBy, setSortBy] = useState('age');
  const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('desc');
  const [page, setPage] = useState(1);

  const { data, isLoading, error } = useArticles({
    health: healthFilter,
    sort: sortBy,
    order: sortOrder,
    page,
    limit: 50,
  });

  if (isLoading) return <div className="text-center py-8">Loading articles...</div>;
  if (error) return <div className="text-red-600 py-8">Error: {error.message}</div>;
  if (!data) return null;

  const handleSort = (column: string) => {
    if (sortBy === column) {
      setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc');
    } else {
      setSortBy(column);
      setSortOrder('desc');
    }
  };

  return (
    <div className="space-y-4">
      {/* Filters */}
      <div className="flex gap-2">
        <button
          onClick={() => setHealthFilter(undefined)}
          className={`px-3 py-1.5 rounded text-sm ${
            !healthFilter ? 'bg-gray-800 text-white' : 'bg-gray-200 text-gray-700'
          }`}
        >
          All
        </button>
        <button
          onClick={() => setHealthFilter('green')}
          className={`px-3 py-1.5 rounded text-sm ${
            healthFilter === 'green' ? 'bg-green-600 text-white' : 'bg-green-100 text-green-800'
          }`}
        >
          Green
        </button>
        <button
          onClick={() => setHealthFilter('yellow')}
          className={`px-3 py-1.5 rounded text-sm ${
            healthFilter === 'yellow' ? 'bg-yellow-600 text-white' : 'bg-yellow-100 text-yellow-800'
          }`}
        >
          Yellow
        </button>
        <button
          onClick={() => setHealthFilter('red')}
          className={`px-3 py-1.5 rounded text-sm ${
            healthFilter === 'red' ? 'bg-red-600 text-white' : 'bg-red-100 text-red-800'
          }`}
        >
          Red
        </button>
      </div>

      {/* Table */}
      <div className="overflow-x-auto">
        <table className="min-w-full bg-white border border-gray-200">
          <thead className="bg-gray-50">
            <tr>
              <th
                onClick={() => handleSort('title')}
                className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider cursor-pointer hover:bg-gray-100"
              >
                Title {sortBy === 'title' && (sortOrder === 'asc' ? '↑' : '↓')}
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                Space
              </th>
              <th
                onClick={() => handleSort('age')}
                className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider cursor-pointer hover:bg-gray-100"
              >
                Age (days) {sortBy === 'age' && (sortOrder === 'asc' ? '↑' : '↓')}
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                Broken Links
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                Health
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-200">
            {data.articles.map((article) => (
              <tr
                key={article.id}
                onClick={() => onArticleClick(article.id)}
                className="hover:bg-gray-50 cursor-pointer"
              >
                <td className="px-6 py-4 whitespace-nowrap">
                  <div className="text-sm font-medium text-gray-900">{article.title}</div>
                  {article.manually_flagged && (
                    <span className="text-xs text-red-600">⚠ Flagged</span>
                  )}
                </td>
                <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                  {article.space_key || '-'}
                </td>
                <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                  {article.effective_age_days}
                </td>
                <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                  {article.broken_link_count > 0 ? (
                    <span className="text-red-600 font-medium">{article.broken_link_count}</span>
                  ) : (
                    '-'
                  )}
                </td>
                <td className="px-6 py-4 whitespace-nowrap">
                  <HealthBadge health={article.health} size="sm" />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Pagination - only show if total articles exceeds page limit */}
      {data.total > data.limit && (
        <div className="flex justify-between items-center mt-4">
          <div className="text-sm text-gray-700">
            Showing {(page - 1) * data.limit + 1} to {Math.min(page * data.limit, data.total)} of{' '}
            {data.total} articles
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => setPage((p) => Math.max(1, p - 1))}
              disabled={page === 1}
              className="px-3 py-1 bg-gray-200 rounded disabled:opacity-50"
            >
              Previous
            </button>
            <button
              onClick={() => setPage((p) => p + 1)}
              // Disable "Next" when current page times limit reaches or exceeds total
              disabled={page * data.limit >= data.total}
              className="px-3 py-1 bg-gray-200 rounded disabled:opacity-50"
            >
              Next
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
