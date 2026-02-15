import { useState } from 'react';
import { useArticleScreenshots, getScreenshotImageUrl } from '../api/client';

interface ScreenshotTimelineProps {
  articleId: string;
}

export function ScreenshotTimeline({ articleId }: ScreenshotTimelineProps) {
  const { data: screenshots, isLoading } = useArticleScreenshots(articleId);
  const [selectedPair, setSelectedPair] = useState<{ current: string; previous: string } | null>(
    null
  );

  if (isLoading) {
    return <div className="text-sm text-gray-500">Loading screenshots...</div>;
  }

  if (!screenshots || screenshots.length === 0) {
    return (
      <div className="text-sm text-gray-500">
        No screenshots captured yet. Screenshots are captured during weekly scans.
      </div>
    );
  }

  const handleCompare = (currentId: string, previousId: string) => {
    setSelectedPair({ current: currentId, previous: previousId });
  };

  return (
    <div className="space-y-4">
      {/* Screenshot timeline */}
      <div className="space-y-3">
        {screenshots.map((screenshot, index) => {
          const hasNext = index < screenshots.length - 1;
          const nextScreenshot = hasNext ? screenshots[index + 1] : null;

          return (
            <div
              key={screenshot.id}
              className={`p-4 rounded border ${
                screenshot.needs_update
                  ? 'bg-yellow-50 border-yellow-300'
                  : 'bg-white border-gray-200'
              }`}
            >
              <div className="flex justify-between items-start">
                <div className="flex-1">
                  <div className="text-sm font-medium">
                    {new Date(screenshot.captured_at).toLocaleString()}
                  </div>
                  {screenshot.drift_distance !== null && screenshot.drift_distance !== undefined && (
                    <div className="text-xs text-gray-600 mt-1">
                      Drift from previous: {screenshot.drift_distance}{' '}
                      {screenshot.needs_update && (
                        <span className="text-yellow-700 font-medium">(significant change)</span>
                      )}
                    </div>
                  )}
                </div>
                <div className="flex gap-2">
                  <button
                    onClick={() => window.open(getScreenshotImageUrl(screenshot.id), '_blank')}
                    className="px-3 py-1 text-sm bg-blue-600 text-white rounded hover:bg-blue-700"
                  >
                    View
                  </button>
                  {hasNext && nextScreenshot && (
                    <button
                      onClick={() => handleCompare(screenshot.id, nextScreenshot.id)}
                      className="px-3 py-1 text-sm bg-gray-600 text-white rounded hover:bg-gray-700"
                    >
                      Compare
                    </button>
                  )}
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* Side-by-side comparison modal */}
      {selectedPair && (
        <div
          className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
          onClick={() => setSelectedPair(null)}
        >
          <div
            className="bg-white rounded-lg max-w-7xl w-full max-h-[90vh] overflow-auto"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="sticky top-0 bg-white border-b p-4 flex justify-between items-center">
              <h3 className="text-lg font-semibold">Screenshot Comparison</h3>
              <button
                onClick={() => setSelectedPair(null)}
                className="px-4 py-2 bg-gray-200 rounded hover:bg-gray-300"
              >
                Close
              </button>
            </div>
            <div className="p-6">
              <div className="grid grid-cols-2 gap-6">
                <div>
                  <h4 className="text-sm font-medium text-gray-700 mb-2">Current</h4>
                  <img
                    src={getScreenshotImageUrl(selectedPair.current)}
                    alt="Current screenshot"
                    className="w-full border border-gray-300 rounded shadow-sm"
                  />
                </div>
                <div>
                  <h4 className="text-sm font-medium text-gray-700 mb-2">Previous</h4>
                  <img
                    src={getScreenshotImageUrl(selectedPair.previous)}
                    alt="Previous screenshot"
                    className="w-full border border-gray-300 rounded shadow-sm"
                  />
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
