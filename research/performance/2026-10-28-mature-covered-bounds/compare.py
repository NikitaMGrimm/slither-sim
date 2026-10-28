"""Complete raw trace byte comparison, also used by the mutation controls."""


def same_bytes(left, right):
    with left.open('rb') as a, right.open('rb') as b:
        while True:
            x, y = a.read(1 << 20), b.read(1 << 20)
            if x != y:
                return False
            if not x:
                return True
