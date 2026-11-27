"""Check the saved work against the desk list. Optionally print and check each A4 page."""
import argparse
import hashlib
import json
import re
import subprocess
import tempfile
from collections import Counter
from html.parser import HTMLParser
from pathlib import Path

HERE = Path(__file__).resolve().parent


class Badges(HTMLParser):
    """Read the actual badge elements, allowing only passive, self-contained markup."""

    def __init__(self):
        super().__init__()
        self.stack = []
        self.cards = []

    def handle_starttag(self, tag, attrs):
        if tag not in 'html head meta title style body main article span div p h1 h2 strong small'.split():
            raise ValueError(f'unexpected element {tag}')
        attributes = dict(attrs)
        if set(attributes) - {'lang', 'charset', 'name', 'content', 'class', 'id', 'style', 'aria-label', 'data-seat'}:
            raise ValueError(f'unexpected attribute on {tag}')
        if tag != 'meta':
            self.stack.append((tag, attributes.get('class', '').split()))
        if tag == 'article':
            self.cards.append({'key': attributes['data-seat'], 'name': [], 'seat': [], 'text': []})

    def handle_endtag(self, tag):
        if tag != 'meta' and (not self.stack or self.stack.pop()[0] != tag):
            raise ValueError(f'unbalanced element {tag}')

    def handle_data(self, text):
        if any(node == 'article' for node, _ in self.stack):
            card = self.cards[-1]
            card['text'].extend(text.split())
            for field in ('name', 'seat'):
                if any(field in classes for _, classes in self.stack):
                    card[field].append(text)


def verify_sheet(path, expected):
    source = path.read_text()
    if re.search(r'@import|url\s*\(|https?://', source, re.I):
        raise ValueError('external resources are outside the print contract')
    parser = Badges()
    parser.feed(source)
    parser.close()
    actual = {}
    for card in parser.cards:
        name, seat = (''.join(card[field]).strip() for field in ('name', 'seat'))
        if seat in actual or seat != card['key'] or Counter(card['text']) != Counter((name + ' ' + seat).split()):
            raise ValueError(f'duplicate seat, extra text or inconsistent badge: {seat}')
        actual[seat] = name
    if actual != expected or parser.stack:
        raise ValueError(f'badge mismatch in {path.name}: {actual}')


def verify_print(browser, source, pdf, expected):
    with tempfile.TemporaryDirectory() as profile:
        subprocess.run([browser, '--headless', '--disable-gpu', '--disable-background-networking',
                        '--no-pdf-header-footer', f'--user-data-dir={profile}',
                        f'--print-to-pdf={pdf}', source.as_uri()],
                       capture_output=True, check=True, timeout=30)
    info = subprocess.check_output(['pdfinfo', str(pdf)], text=True)
    if not re.search(r'^Pages:\s+1$', info, re.M) or '(A4)' not in info:
        raise ValueError(f'{pdf.name} did not fit on one A4 page')
    printed = subprocess.check_output(['pdftotext', '-layout', str(pdf), '-'], text=True)
    words = ' '.join(f'{seat} {name}' for seat, name in expected.items()).split()
    if Counter(printed.split()) != Counter(words):
        raise ValueError(f'printed labels differ from desk list in {pdf.name}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser', help='Chromium executable, with Poppler on PATH')
    parser.add_argument('--pdf-dir', type=Path, help='directory to retain verified PDFs')
    args = parser.parse_args()
    expected = json.loads((HERE / 'expected.json').read_text())
    root = HERE / 'results'
    record = json.loads((root / 'runs.json').read_text())
    pairs = {(task, arm) for task in expected for arm in ('empty', 'live', 'static')}
    if len(record['runs']) != len(pairs):
        raise ValueError('incomplete or duplicated trials')
    with tempfile.TemporaryDirectory() as tmp:
        pdf_dir = (args.pdf_dir or Path(tmp)).resolve()
        pdf_dir.mkdir(parents=True, exist_ok=True)
        for run in record['runs']:
            task, arm = run['task'], run['arm']
            pairs.remove((task, arm))
            source = root / run['output']
            if hashlib.sha256(source.read_bytes()).hexdigest() != run['sha256']:
                raise ValueError(f'{source.name} differs from the recorded output')
            if arm == 'live' and task == 'new-event':
                if not any(c['matched_id'] == record['banked_id'] for c in run['calls']):
                    raise ValueError('missing witnessed retrieval of the taught method')
            if arm == 'empty' and task == 'new-event':
                if source.read_text().strip() != 'NEED DESK NOTE' or any(c['matched_id'] for c in run['calls']):
                    raise ValueError('empty-bank control changed')
                print(f'{arm}/{task}: requested the missing note')
                continue
            verify_sheet(source, expected[task])
            if args.browser:
                verify_print(args.browser, source, pdf_dir / (source.stem + '.pdf'), expected[task])
            print(f'{arm}/{task}: {len(expected[task])} correct badges' + (' on one A4 page' if args.browser else ''))

        native = HERE / 'native'
        trials = json.loads((native / 'runs.json').read_text())
        pending = {('tools-only', 'new-event'), ('skill', 'new-event'), ('skill', 'changed-brief')}
        for trial in trials:
            setup, task = trial['setup'], trial['task']
            pending.remove((setup, task))
            source = native / trial['answer']
            if hashlib.sha256(source.read_bytes()).hexdigest() != trial['sha256']:
                raise ValueError(f'{source.name} differs from the native run')
            lesson = json.loads((native / f'{setup}-lesson.json').read_text())
            if trial['matched_id'] != (lesson['id'] if task == 'new-event' else None):
                raise ValueError(f'unexpected recall for {setup}/{task}')
            if setup == 'tools-only':
                if source.read_text().strip() != 'NEED DESK NOTE':
                    raise ValueError('tools-only miss changed')
                print('native/tools-only: retrieved the lesson and still requested the note')
                continue
            verify_sheet(source, expected[task])
            if args.browser:
                verify_print(args.browser, source, pdf_dir / (source.stem + '.pdf'), expected[task])
            print(f'native/{setup}/{task}: {len(expected[task])} correct badges')
        if pending:
            raise ValueError(f'missing native trials: {pending}')


if __name__ == '__main__':
    main()
