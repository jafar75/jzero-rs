// Demonstrates string concatenation and String.valueOf.
// Run: j0 tests/examples/greet.java --run a b c
public class greet {
    public static void main(String argv[]) {
        String greeting;
        String msg;
        int n;
        n = argv.length;
        greeting = "hello, jzero!";
        msg = "running with " + String.valueOf(n) + " args";
        System.out.println(greeting);
        System.out.println(msg);
    }
}