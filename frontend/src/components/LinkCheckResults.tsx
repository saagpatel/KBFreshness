import { useArticleLinks } from '../api/client';

interface LinkCheckResultsProps {
  articleId: string;
}

export function LinkCheckResults({ articleId }: LinkCheckResultsProps) {
  const { data: links, isLoading } = useArticleLinks(articleId);

  if (isLoading) return <div className="text-sm text-gray-500">Loading link checks...</div>;
  if (!links || links.length === 0) {
    return <div className="text-sm text-gray-500">No links found in this article.</div>;
  }

  const brokenLinks = links.filter((l) => l.is_broken);
  const workingLinks = links.filter((l) => !l.is_broken);

  return (
    <div className="space-y-4">
      {brokenLinks.length > 0 && (
        <div>
          <h3 className="text-md font-semibold text-red-700 mb-2">
            Broken Links ({brokenLinks.length})
          </h3>
          <div className="space-y-2">
            {brokenLinks.map((link) => (
              <div key={link.id} className="p-3 bg-red-50 border border-red-200 rounded">
                <div className="flex justify-between items-start">
                  <a
                    href={link.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-sm text-blue-600 hover:underline break-all flex-1"
                  >
                    {link.url}
                  </a>
                  <span className="ml-2 px-2 py-0.5 bg-red-600 text-white text-xs rounded">
                    {link.status_code || 'ERROR'}
                  </span>
                </div>
                {link.error_message && (
                  <div className="text-xs text-red-600 mt-1">{link.error_message}</div>
                )}
                <div className="text-xs text-gray-500 mt-1">
                  Checked: {new Date(link.checked_at).toLocaleString()}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {workingLinks.length > 0 && (
        <details className="cursor-pointer">
          <summary className="text-sm font-medium text-gray-700 hover:text-gray-900">
            Working Links ({workingLinks.length})
          </summary>
          <div className="mt-2 space-y-1 pl-4">
            {workingLinks.map((link) => (
              <div key={link.id} className="text-sm">
                <a
                  href={link.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-blue-600 hover:underline break-all"
                >
                  {link.url}
                </a>
                <span className="ml-2 text-green-600 text-xs">✓ {link.status_code}</span>
              </div>
            ))}
          </div>
        </details>
      )}
    </div>
  );
}
