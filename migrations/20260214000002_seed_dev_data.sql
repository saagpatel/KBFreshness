-- Seed data for development: 3 sample articles (1 green, 1 yellow, 1 red)

-- Green article: Fresh, no broken links
INSERT INTO articles (id, title, url, source, source_id, space_key, last_modified_at, last_modified_by, version_number)
VALUES (
    '550e8400-e29b-41d4-a716-446655440001',
    'Getting Started with VPN',
    'https://kb.example.com/pages/getting-started-vpn',
    'confluence',
    'PAGE001',
    'KB',
    NOW() - INTERVAL '30 days',
    'John Doe',
    5
);

-- Yellow article: Slightly old, 1 broken link
INSERT INTO articles (id, title, url, source, source_id, space_key, last_modified_at, last_modified_by, version_number)
VALUES (
    '550e8400-e29b-41d4-a716-446655440002',
    'MacOS System Preferences Guide',
    'https://kb.example.com/pages/macos-system-prefs',
    'confluence',
    'PAGE002',
    'KB',
    NOW() - INTERVAL '120 days',
    'Jane Smith',
    12
);

-- Red article: Very old, multiple broken links
INSERT INTO articles (id, title, url, source, source_id, space_key, last_modified_at, last_modified_by, version_number, manually_flagged)
VALUES (
    '550e8400-e29b-41d4-a716-446655440003',
    'Windows 7 Troubleshooting',
    'https://kb.example.com/pages/windows-7-troubleshooting',
    'confluence',
    'PAGE003',
    'KB',
    NOW() - INTERVAL '500 days',
    'Bob Johnson',
    8,
    TRUE
);

-- Link checks for yellow article (1 broken)
INSERT INTO link_checks (article_id, url, status_code, is_broken, checked_at)
VALUES (
    '550e8400-e29b-41d4-a716-446655440002',
    'https://support.apple.com/old-guide',
    404,
    TRUE,
    NOW() - INTERVAL '1 day'
);

-- Link checks for red article (3 broken)
INSERT INTO link_checks (article_id, url, status_code, is_broken, error_message, checked_at)
VALUES
    ('550e8400-e29b-41d4-a716-446655440003', 'https://microsoft.com/windows7/eol', 404, TRUE, 'Not Found', NOW() - INTERVAL '1 day'),
    ('550e8400-e29b-41d4-a716-446655440003', 'https://docs.oldservice.com/api', NULL, TRUE, 'Connection timeout', NOW() - INTERVAL '1 day'),
    ('550e8400-e29b-41d4-a716-446655440003', 'https://defunct.example.com', NULL, TRUE, 'DNS resolution failed', NOW() - INTERVAL '1 day');
