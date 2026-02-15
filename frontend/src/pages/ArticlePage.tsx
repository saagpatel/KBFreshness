import { useState } from 'react';
import { useArticle, useReviewArticle, useFlagArticle } from '../api/client';
import { HealthBadge } from '../components/HealthBadge';
import { LinkCheckResults } from '../components/LinkCheckResults';
import { ScreenshotTimeline } from '../components/ScreenshotTimeline';
import { TicketPatterns } from '../components/TicketPatterns';
import { ReviewDialog } from '../components/ReviewDialog';
import { formatDate } from '../utils/dates';

interface ArticlePageProps {
  articleId: string;
  onBack: () => void;
}

export function ArticlePage({ articleId, onBack }: ArticlePageProps) {
  const { data: article, isLoading, error } = useArticle(articleId);
  const reviewMutation = useReviewArticle();
  const flagMutation = useFlagArticle();
  const [showReviewDialog, setShowReviewDialog] = useState(false);

  if (isLoading) return <div className="p-6">Loading...</div>;
  if (error) return <div className="p-6 text-red-600">Error: {error.message}</div>;
  if (!article) return null;

  const handleReview = () => {
    setShowReviewDialog(true);
  };

  const handleReviewSubmit = (name: string) => {
    reviewMutation.mutate(
      { id: article.id, reviewed_by: name },
      {
        onSuccess: () => setShowReviewDialog(false),
      }
    );
  };

  const handleFlag = () => {
    const newState = !article.manually_flagged;
    if (
      confirm(
        newState
          ? 'Flag this article as requiring urgent attention?'
          : 'Remove the urgent flag from this article?'
      )
    ) {
      flagMutation.mutate({ id: article.id, flagged: newState });
    }
  };

  return (
    <div className="min-h-screen bg-gray-100 p-6">
      <div className="max-w-5xl mx-auto">
        <button
          onClick={onBack}
          className="mb-4 text-blue-600 hover:text-blue-800 flex items-center"
        >
          ← Back to Dashboard
        </button>

        <div className="bg-white p-6 rounded-lg shadow">
          <div className="flex justify-between items-start mb-6">
            <div>
              <h1 className="text-2xl font-bold mb-2">{article.title}</h1>
              <a
                href={article.url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-blue-600 hover:underline text-sm"
              >
                {article.url} ↗
              </a>
            </div>
            <HealthBadge health={article.health} size="lg" />
          </div>

          {/* Metadata */}
          <div className="grid grid-cols-2 gap-4 mb-6 p-4 bg-gray-50 rounded">
            <div>
              <div className="text-sm text-gray-600">Source</div>
              <div className="font-medium capitalize">{article.source}</div>
            </div>
            {article.space_key && (
              <div>
                <div className="text-sm text-gray-600">Space</div>
                <div className="font-medium">{article.space_key}</div>
              </div>
            )}
            <div>
              <div className="text-sm text-gray-600">Last Modified</div>
              <div className="font-medium">
                {formatDate(article.last_modified_at)}
                {article.last_modified_by && ` by ${article.last_modified_by}`}
              </div>
            </div>
            <div>
              <div className="text-sm text-gray-600">Age</div>
              <div className="font-medium">{article.effective_age_days} days</div>
            </div>
            <div>
              <div className="text-sm text-gray-600">Version</div>
              <div className="font-medium">v{article.version_number}</div>
            </div>
            {article.reviewed_at && (
              <div>
                <div className="text-sm text-gray-600">Last Reviewed</div>
                <div className="font-medium">
                  {formatDate(article.reviewed_at)}
                  {article.reviewed_by && ` by ${article.reviewed_by}`}
                </div>
              </div>
            )}
          </div>

          {/* Actions */}
          <div className="flex gap-3 mb-6">
            <button
              onClick={handleReview}
              disabled={reviewMutation.isPending}
              className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50"
            >
              {reviewMutation.isPending ? 'Saving...' : 'Mark as Reviewed'}
            </button>
            <button
              onClick={handleFlag}
              disabled={flagMutation.isPending}
              className={`px-4 py-2 rounded ${
                article.manually_flagged
                  ? 'bg-gray-200 text-gray-700 hover:bg-gray-300'
                  : 'bg-red-600 text-white hover:bg-red-700'
              } disabled:opacity-50`}
            >
              {flagMutation.isPending
                ? 'Saving...'
                : article.manually_flagged
                  ? 'Remove Flag'
                  : 'Flag for Attention'}
            </button>
          </div>

          {/* Link check results */}
          <div className="border-t pt-6">
            <h2 className="text-lg font-semibold mb-4">Link Validation</h2>
            <LinkCheckResults articleId={article.id} />
          </div>

          {/* Screenshot timeline */}
          <div className="border-t pt-6">
            <h2 className="text-lg font-semibold mb-4">Screenshot History</h2>
            <p className="text-sm text-gray-600 mb-4">
              Visual snapshots captured during weekly scans. Significant changes are highlighted.
            </p>
            <ScreenshotTimeline articleId={article.id} />
          </div>

          {/* Ticket patterns */}
          <div className="border-t pt-6">
            <h2 className="text-lg font-semibold mb-4">Related Support Tickets</h2>
            <p className="text-sm text-gray-600 mb-4">
              Patterns detected from recent support tickets that may indicate this article needs updating.
            </p>
            <TicketPatterns patterns={article.ticket_patterns || []} />
          </div>
        </div>

        <ReviewDialog
          isOpen={showReviewDialog}
          onClose={() => setShowReviewDialog(false)}
          onSubmit={handleReviewSubmit}
          isPending={reviewMutation.isPending}
        />
      </div>
    </div>
  );
}
