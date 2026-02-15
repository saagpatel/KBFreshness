/**
 * Consistent date formatting utilities across the application
 */

/**
 * Format a date string to locale date string
 * Example: "2024-01-15" -> "1/15/2024" (US locale)
 */
export function formatDate(dateString: string): string {
  return new Date(dateString).toLocaleDateString();
}

/**
 * Format a date string to locale date and time string
 * Example: "2024-01-15T10:30:00Z" -> "1/15/2024, 10:30:00 AM" (US locale)
 */
export function formatDateTime(dateString: string): string {
  return new Date(dateString).toLocaleString();
}

/**
 * Format a date string with custom options
 */
export function formatDateCustom(dateString: string, options: Intl.DateTimeFormatOptions): string {
  return new Date(dateString).toLocaleDateString(undefined, options);
}
