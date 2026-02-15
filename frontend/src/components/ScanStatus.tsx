import { useScans, useTriggerScan } from '../api/client';
import { formatDateTime } from '../utils/dates';

export function ScanStatus() {
  const { data: scans, isLoading } = useScans(1);
  const triggerScan = useTriggerScan();

  if (isLoading) return null;

  const latestScan = scans?.[0];

  return (
    <div className="bg-white p-4 rounded-lg shadow mb-6">
      <div className="flex justify-between items-center">
        <div>
          <h3 className="text-lg font-semibold">Scan Status</h3>
          {latestScan && (
            <div className="text-sm text-gray-600 mt-1">
              {latestScan.status === 'running' ? (
                <span className="text-blue-600">Scan in progress...</span>
              ) : (
                <>
                  Last scan: {formatDateTime(latestScan.started_at)} |{' '}
                  {latestScan.articles_scanned} articles, {latestScan.broken_links_found} broken
                  links
                </>
              )}
            </div>
          )}
        </div>
        <button
          onClick={() => triggerScan.mutate()}
          disabled={triggerScan.isPending || latestScan?.status === 'running'}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
        >
          {triggerScan.isPending ? 'Triggering...' : 'Trigger Scan'}
        </button>
      </div>
      {triggerScan.isError && (
        <div className="mt-2 text-sm text-red-600">
          Error: {triggerScan.error instanceof Error ? triggerScan.error.message : 'Unknown error'}
        </div>
      )}
    </div>
  );
}
