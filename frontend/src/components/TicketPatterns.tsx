import { TicketPattern } from '../types';

interface TicketPatternsProps {
  patterns: TicketPattern[];
}

export function TicketPatterns({ patterns }: TicketPatternsProps) {
  if (patterns.length === 0) {
    return (
      <div className="text-sm text-gray-500">
        No related support tickets found. Ticket patterns are updated weekly.
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {patterns.map((pattern) => (
        <div key={pattern.id} className="p-4 bg-blue-50 border border-blue-200 rounded">
          <div className="flex justify-between items-start mb-2">
            <div className="flex items-center gap-2">
              <span className="px-2 py-1 bg-blue-600 text-white text-xs font-semibold rounded">
                {pattern.ticket_count} {pattern.ticket_count === 1 ? 'ticket' : 'tickets'}
              </span>
              <span className="text-sm text-gray-600">
                {new Date(pattern.detected_at).toLocaleDateString()}
              </span>
            </div>
          </div>

          {/* Keywords */}
          {pattern.keywords && pattern.keywords.length > 0 && (
            <div className="flex flex-wrap gap-2 mb-3">
              {pattern.keywords.map((keyword, idx) => (
                <span
                  key={idx}
                  className="px-2 py-1 bg-white border border-blue-300 text-blue-700 text-xs rounded"
                >
                  {keyword}
                </span>
              ))}
            </div>
          )}

          {/* Suggestion */}
          {pattern.suggested_update && (
            <div className="text-sm text-gray-700">
              <span className="font-medium">Suggestion:</span>{' '}
              {pattern.suggested_update}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
