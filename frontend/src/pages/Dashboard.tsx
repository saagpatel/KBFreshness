import { useArticleStats } from '../api/client';
import { ArticleTable } from '../components/ArticleTable';
import { ScanStatus } from '../components/ScanStatus';

interface DashboardProps {
  onArticleClick: (id: string) => void;
}

export function Dashboard({ onArticleClick }: DashboardProps) {
  // Use dedicated stats endpoint
  const { data: stats } = useArticleStats();
  const statsData = stats || { total: 0, green: 0, yellow: 0, red: 0 };

  return (
    <div className="min-h-screen bg-gray-100 p-6">
      <div className="max-w-7xl mx-auto">
        <h1 className="text-3xl font-bold text-gray-900 mb-6">KB Freshness Detector</h1>

        <ScanStatus />

        {/* Summary cards */}
        <div className="grid grid-cols-4 gap-4 mb-6">
          <div className="bg-white p-6 rounded-lg shadow">
            <div className="text-2xl font-bold">{statsData.total}</div>
            <div className="text-gray-600">Total Articles</div>
          </div>
          <div className="bg-green-50 p-6 rounded-lg shadow border-2 border-green-200">
            <div className="text-2xl font-bold text-green-800">{statsData.green}</div>
            <div className="text-green-700">Green</div>
          </div>
          <div className="bg-yellow-50 p-6 rounded-lg shadow border-2 border-yellow-200">
            <div className="text-2xl font-bold text-yellow-800">{statsData.yellow}</div>
            <div className="text-yellow-700">Yellow</div>
          </div>
          <div className="bg-red-50 p-6 rounded-lg shadow border-2 border-red-200">
            <div className="text-2xl font-bold text-red-800">{statsData.red}</div>
            <div className="text-red-700">Red</div>
          </div>
        </div>

        {/* Article table */}
        <div className="bg-white p-6 rounded-lg shadow">
          <h2 className="text-xl font-semibold mb-4">Articles</h2>
          <ArticleTable onArticleClick={onArticleClick} />
        </div>
      </div>
    </div>
  );
}
