import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { Article, ArticleDetail, ArticleListResponse, ScanRun, LinkCheck, Screenshot } from '../types';

// Validate and normalize API base URL
function getApiBase(): string {
  const envUrl = import.meta.env.VITE_API_URL;

  if (!envUrl) {
    return '/api';
  }

  // Validate URL format
  try {
    const url = new URL(envUrl, window.location.origin);
    // Return path only for relative URLs, full URL for absolute
    return envUrl.startsWith('http') ? envUrl : url.pathname;
  } catch (e) {
    throw new Error(`Invalid VITE_API_URL format: ${envUrl}. Must be a valid URL or path.`);
  }
}

const API_BASE = getApiBase();

async function fetcher<T>(url: string, options?: RequestInit): Promise<T> {
  const response = await fetch(`${API_BASE}${url}`, options);
  if (!response.ok) {
    // Try to parse JSON error first
    let errorMessage = 'Request failed';
    try {
      const errorData = await response.json();
      errorMessage = errorData.error || errorData.message || response.statusText;
    } catch {
      // If JSON parsing fails, try to get text response
      try {
        const errorText = await response.text();
        errorMessage = errorText || response.statusText;
      } catch {
        errorMessage = response.statusText;
      }
    }
    throw new Error(errorMessage);
  }
  return response.json();
}

// Articles
export function useArticles(params?: {
  health?: string;
  space?: string;
  sort?: string;
  order?: string;
  page?: number;
  limit?: number;
}) {
  const queryString = new URLSearchParams(
    Object.entries(params || {})
      .filter(([, value]) => value !== undefined)
      .map(([key, value]) => [key, String(value)])
  ).toString();

  return useQuery({
    queryKey: ['articles', params],
    queryFn: () => fetcher<ArticleListResponse>(`/articles?${queryString}`),
  });
}

export interface ArticleStats {
  total: number;
  green: number;
  yellow: number;
  red: number;
}

export function useArticleStats(params?: {
  health?: string;
  space?: string;
}) {
  const queryString = new URLSearchParams(
    Object.entries(params || {})
      .filter(([, value]) => value !== undefined)
      .map(([key, value]) => [key, String(value)])
  ).toString();

  return useQuery({
    queryKey: ['articles', 'stats', params],
    queryFn: () => fetcher<ArticleStats>(`/articles/stats?${queryString}`),
  });
}

export function useArticle(id: string) {
  return useQuery({
    queryKey: ['articles', id],
    queryFn: () => fetcher<ArticleDetail>(`/articles/${id}`),
    enabled: !!id,
  });
}

export function useReviewArticle() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, reviewed_by }: { id: string; reviewed_by: string }) =>
      fetcher<Article>(`/articles/${id}/review`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ reviewed_by }),
      }),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['articles'] });
      queryClient.setQueryData(['articles', data.id], data);
    },
  });
}

export function useFlagArticle() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, flagged }: { id: string; flagged: boolean }) =>
      fetcher<Article>(`/articles/${id}/flag`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ flagged }),
      }),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['articles'] });
      queryClient.setQueryData(['articles', data.id], data);
    },
  });
}

// Scans
export function useScans(limit = 10) {
  return useQuery({
    queryKey: ['scans', limit],
    queryFn: () => fetcher<ScanRun[]>(`/scans?limit=${limit}`),
  });
}

export function useTriggerScan() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () =>
      fetcher<ScanRun>('/scans/trigger', { method: 'POST' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['articles'] });
      queryClient.invalidateQueries({ queryKey: ['scans'] });
    },
  });
}

// Link checks
export function useArticleLinks(articleId: string) {
  return useQuery({
    queryKey: ['articles', articleId, 'links'],
    queryFn: () => fetcher<LinkCheck[]>(`/articles/${articleId}/links`),
    enabled: !!articleId,
  });
}

// Screenshots
export function useArticleScreenshots(articleId: string) {
  return useQuery({
    queryKey: ['articles', articleId, 'screenshots'],
    queryFn: () => fetcher<Screenshot[]>(`/articles/${articleId}/screenshots`),
    enabled: !!articleId,
  });
}

export function getScreenshotImageUrl(screenshotId: string): string {
  const API_BASE = import.meta.env.VITE_API_URL ?? '/api';
  return `${API_BASE}/screenshots/${screenshotId}/image`;
}
