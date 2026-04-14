// Countdown from argv.length to 1, printing a message each step.
// Run: j0 tests/examples/countdown.java --run a b c a b
// With 5 args: prints "going down..." 5 times, then "liftoff!"
public class countdown {
    public static void main(String argv[]) {
        int n;
        n = argv.length;
        while (n > 0) {
            System.out.println("going down...");
            n = n - 1;
        }
        System.out.println("liftoff!");
    }
}