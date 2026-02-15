import { useState } from 'react';

interface ReviewDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (name: string) => void;
  isPending: boolean;
}

export function ReviewDialog({ isOpen, onClose, onSubmit, isPending }: ReviewDialogProps) {
  const [name, setName] = useState('');
  const [error, setError] = useState('');

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();

    // Validate input
    const trimmedName = name.trim();
    if (!trimmedName) {
      setError('Name is required');
      return;
    }

    if (trimmedName.length > 100) {
      setError('Name must be 100 characters or less');
      return;
    }

    // Sanitize: only allow alphanumeric, spaces, and basic punctuation
    if (!/^[a-zA-Z0-9\s\-_\.]+$/.test(trimmedName)) {
      setError('Name contains invalid characters');
      return;
    }

    setError('');
    onSubmit(trimmedName);
  };

  const handleClose = () => {
    setName('');
    setError('');
    onClose();
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white rounded-lg p-6 max-w-md w-full mx-4">
        <h3 className="text-lg font-semibold mb-4">Mark Article as Reviewed</h3>

        <form onSubmit={handleSubmit}>
          <div className="mb-4">
            <label htmlFor="reviewer-name" className="block text-sm font-medium text-gray-700 mb-2">
              Your name
            </label>
            <input
              id="reviewer-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Enter your name"
              className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
              disabled={isPending}
              autoFocus
            />
            {error && (
              <p className="mt-1 text-sm text-red-600">{error}</p>
            )}
          </div>

          <div className="flex gap-3 justify-end">
            <button
              type="button"
              onClick={handleClose}
              disabled={isPending}
              className="px-4 py-2 text-gray-700 bg-gray-100 rounded hover:bg-gray-200 disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isPending}
              className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50"
            >
              {isPending ? 'Saving...' : 'Confirm Review'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
