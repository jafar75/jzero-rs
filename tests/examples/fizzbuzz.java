// FizzBuzz — prints fizz, buzz, fizzbuzz, or the iteration number.
// Counts from 1 to argv.length.
// Run: j0 tests/examples/fizzbuzz.java --run a b c a b c a b c a b c a b c
// (15 args → classic fizzbuzz output)
public class fizzbuzz {
    public static void main(String argv[]) {
        int n;
        int i;
        n = argv.length;
        i = 1;
        while (i <= n) {
            if (i % 15 < 1) {
                System.out.println("fizzbuzz");
            } else {
                if (i % 3 < 1) {
                    System.out.println("fizz");
                } else {
                    if (i % 5 < 1) {
                        System.out.println("buzz");
                    } else {
                        System.out.println(String.valueOf(i));
                    }
                }
            }
            i = i + 1;
        }
    }
}