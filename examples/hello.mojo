def main():
    print("Hello from Mojo")
    var x = 1.0
    var y = 2.0
    var z = add(x, y)
    print(z)

def add(a: Float64, b: Float64) -> Float64:
    return a + b
