// Compute Fibonacci numbers iteratively.
// argv.length determines how many to print.
// Run: j0 tests/examples/fibonacci.java --run a b c d a b c d a b
// (10 args → prints first 10 Fibonacci numbers)
public class fibonacci {
    public static void main(String argv[]) {
        int n;
        int a;
        int b;
        int tmp;
        int i;
        n = argv.length;
        a = 0;
        b = 1;
        i = 0;
        while (i < n) {
            System.out.println(String.valueOf(a));
            tmp = a + b;
            a = b;
            b = tmp;
            i = i + 1;
        }
    }
}