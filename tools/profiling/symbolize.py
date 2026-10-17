import sys, subprocess, collections, re
samples_path, binary = sys.argv[1], sys.argv[2]
lines = open(samples_path).read().split('\n')
slide = int(lines[0].split()[1], 16)
load = 0x100000000 + slide
samples = [l.split() for l in lines[1:] if l.strip()]
addrs = sorted({a for s in samples for a in s})
open(samples_path + '.addrs', 'w').write('\n'.join('0x' + a for a in addrs))
out = subprocess.run(['atos', '-o', binary, '-l', hex(load), '-f', samples_path + '.addrs'], capture_output=True, text=True).stdout.split('\n')
names = {}
for a, n in zip(addrs, out):
    n = re.sub(r' \(in [^)]*\)', '', n)
    n = re.sub(r' \+ \d+$', '', n)
    n = re.sub(r'::h[0-9a-f]{16}$', '', n)
    n = re.sub(r'\s*\([^()]*\.rs:\d+\)$', '', n)
    names[a] = n
selfc = collections.Counter(); incl = collections.Counter()
for s in samples:
    fr = [names[a] for a in s]
    selfc[fr[0]] += 1
    for n in set(fr):
        incl[n] += 1
N = len(samples)
print('samples', N)
print('== self')
for n, c in selfc.most_common(25): print('%5.1f%% %s' % (100*c/N, n[:170]))
print('== inclusive (lrr frames)')
k = 0
for n, c in incl.most_common(400):
    if 'lrr' in n and not n.startswith('std::') :
        print('%5.1f%% %s' % (100*c/N, n[:170])); k += 1
    if k >= 60: break
