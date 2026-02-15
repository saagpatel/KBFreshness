import { useState } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Dashboard } from './pages/Dashboard';
import { ArticlePage } from './pages/ArticlePage';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

function App() {
  const [currentView, setCurrentView] = useState<{ type: 'dashboard' } | { type: 'article'; id: string }>({
    type: 'dashboard',
  });

  return (
    <QueryClientProvider client={queryClient}>
      {currentView.type === 'dashboard' ? (
        <Dashboard onArticleClick={(id) => setCurrentView({ type: 'article', id })} />
      ) : (
        <ArticlePage
          articleId={currentView.id}
          onBack={() => setCurrentView({ type: 'dashboard' })}
        />
      )}
    </QueryClientProvider>
  );
}

export default App;
