import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { AlertCircle, CheckCircle2, Calculator, Share2 } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import Navbar from "@/components/Navbar";
import Footer from "@/components/Footer";
import { Link, useSearchParams } from "react-router-dom";
import { toast } from "sonner";

// SHA256 implementation for browser
async function sha256(message: string): Promise<ArrayBuffer> {
  const msgBuffer = new TextEncoder().encode(message);
  const hashBuffer = await crypto.subtle.digest("SHA-256", msgBuffer);
  return hashBuffer;
}

// Convert ArrayBuffer to hex string
function bufferToHex(buffer: ArrayBuffer): string {
  return Array.from(new Uint8Array(buffer))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

interface VerificationResult {
  isValid: boolean;
  rolledNumber: number;
  targetNumber: number;
  isWin: boolean;
  multiplier: number;
  winChance: number;
  hashHex: string;
  error?: string;
}

export default function GameVerify() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [txHash, setTxHash] = useState(searchParams.get("tx") || "");
  const [nonce, setNonce] = useState(searchParams.get("nonce") || "");
  const [multiplier, setMultiplier] = useState(
    searchParams.get("multiplier") || "",
  );
  const [verificationResult, setVerificationResult] =
    useState<VerificationResult | null>(null);
  const [isVerifying, setIsVerifying] = useState(false);

  // Update URL params when form values change
  useEffect(() => {
    const params = new URLSearchParams();
    if (txHash) params.set("tx", txHash);
    if (nonce) params.set("nonce", nonce);
    if (multiplier) params.set("multiplier", multiplier);
    setSearchParams(params);
  }, [txHash, nonce, multiplier, setSearchParams]);

  // Auto-verify if all params are present on load
  useEffect(() => {
    if (
      searchParams.get("tx") &&
      searchParams.get("nonce") &&
      searchParams.get("multiplier")
    ) {
      verifyGame();
    }
  }, []); // Only run on mount

  const shareResult = () => {
    const url = window.location.href;
    navigator.clipboard.writeText(url);
    toast.success("Verification link copied to clipboard!");
  };

  const verifyGame = async () => {
    if (!txHash.trim() || !nonce.trim() || !multiplier.trim()) {
      setVerificationResult({
        isValid: false,
        rolledNumber: 0,
        targetNumber: 0,
        isWin: false,
        multiplier: 0,
        winChance: 0,
        hashHex: "",
        error: "Please fill in all fields",
      });
      return;
    }

    setIsVerifying(true);

    try {
      // Parse multiplier
      const multiplierValue = parseFloat(multiplier);
      if (isNaN(multiplierValue) || multiplierValue <= 0) {
        throw new Error("Invalid multiplier value");
      }

      // Replicate the game logic from transaction_processor.rs
      // hash_input = format!("{}{}", current_nonce, outpoint.outpoint.txid);
      const hashInput = `${nonce}${txHash}`;

      // SHA256 hash
      const hashBuffer = await sha256(hashInput);
      const hashBytes = new Uint8Array(hashBuffer);
      const hashHex = bufferToHex(hashBuffer);

      // Use first 2 bytes as u16 for randomness (0-65535 range)
      // let random_value = u16::from_be_bytes([hash_bytes[0], hash_bytes[1]]);
      const randomValue = (hashBytes[0] << 8) | hashBytes[1]; // Big-endian u16
      const rolledNumber = randomValue;

      // Calculate target number based on multiplier
      // From the frontend: target_number: (65536.0 * 1000.0 / multiplier.multiplier() as f64) as i64
      const targetNumber = Math.floor(
        (65536.0 * 1000.0) / (multiplierValue * 100),
      );

      // Check if player wins: rolled_number < target_number
      const isWin = rolledNumber < targetNumber;

      // Calculate win chance percentage
      const winChance = (targetNumber / 65536) * 100;

      setVerificationResult({
        isValid: true,
        rolledNumber,
        targetNumber,
        isWin,
        multiplier: multiplierValue,
        winChance,
        hashHex,
      });
    } catch (error) {
      setVerificationResult({
        isValid: false,
        rolledNumber: 0,
        targetNumber: 0,
        isWin: false,
        multiplier: 0,
        winChance: 0,
        hashHex: "",
        error: error instanceof Error ? error.message : "Verification failed",
      });
    } finally {
      setIsVerifying(false);
    }
  };

  return (
    <div className="min-h-screen bg-background">
      <Navbar />

      <div className="max-w-4xl mx-auto p-6 space-y-8">
        {/* Header */}
        <div className="text-center space-y-4">
          <h1 className="text-4xl font-bold bg-gradient-to-r from-primary to-orange-500 bg-clip-text text-transparent">
            Game Verification
          </h1>
          <p className="text-muted-foreground max-w-2xl mx-auto">
            Verify any Satoshi Dice game result by providing the transaction
            hash and nonce. Our games are provably fair and fully transparent.
          </p>
          <Link to="/game">
            <Button variant="ghost" size="sm">
              ← Back to Game
            </Button>
          </Link>
        </div>

        {/* Verification Form and Step-by-Step */}
        <div className="grid gap-6 lg:grid-cols-2">
          {/* Form */}
          <Card className="bg-gradient-to-br from-card to-card/50 border-primary/20">
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Calculator className="w-5 h-5" />
                Verify Game Result
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="grid gap-4">
                <div className="space-y-2">
                  <Label htmlFor="txhash">Transaction Hash</Label>
                  <Input
                    id="txhash"
                    placeholder="Enter the transaction hash (txid)"
                    value={txHash}
                    onChange={(e) => setTxHash(e.target.value)}
                    className="font-mono text-sm"
                  />
                  <p className="text-xs text-muted-foreground">
                    The Bitcoin transaction ID from your game
                  </p>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="nonce">Nonce</Label>
                  <Input
                    id="nonce"
                    placeholder="Enter the nonce used for this game"
                    value={nonce}
                    onChange={(e) => setNonce(e.target.value)}
                    className="font-mono text-sm"
                  />
                  <p className="text-xs text-muted-foreground">
                    The random nonce that was active when your game was
                    processed
                  </p>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="multiplier">Multiplier</Label>
                  <Input
                    id="multiplier"
                    placeholder="Enter the multiplier (e.g., 2.0, 10.0)"
                    value={multiplier}
                    onChange={(e) => setMultiplier(e.target.value)}
                    type="number"
                    step="0.01"
                    min="1"
                  />
                  <p className="text-xs text-muted-foreground">
                    The payout multiplier for the game address you sent to
                  </p>
                </div>
              </div>

              <Button
                onClick={verifyGame}
                disabled={isVerifying}
                className="w-full"
                size="lg"
              >
                {isVerifying ? "Verifying..." : "Verify Game"}
              </Button>
            </CardContent>
          </Card>

          {/* Step-by-Step Explanation */}
          <Card className="bg-gradient-to-br from-card to-card/50 border-primary/20">
            <CardHeader>
              <CardTitle>What's Happening?</CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-4">
                <div className="flex gap-3">
                  <div className="flex-shrink-0 w-8 h-8 bg-primary/10 rounded-full flex items-center justify-center text-sm font-semibold">
                    1
                  </div>
                  <div>
                    <h4 className="font-semibold mb-1">Combine Inputs</h4>
                    <p className="text-sm text-muted-foreground">
                      We concatenate the nonce + transaction hash to create a
                      unique string:
                      {txHash && nonce && (
                        <code className="block mt-1 p-2 bg-card/50 rounded text-xs break-all">
                          {nonce}
                          {txHash}
                        </code>
                      )}
                    </p>
                  </div>
                </div>

                <div className="flex gap-3">
                  <div className="flex-shrink-0 w-8 h-8 bg-primary/10 rounded-full flex items-center justify-center text-sm font-semibold">
                    2
                  </div>
                  <div>
                    <h4 className="font-semibold mb-1">SHA256 Hash</h4>
                    <p className="text-sm text-muted-foreground">
                      The combined string is hashed using SHA256, creating a
                      32-byte (64 character) hash that's cryptographically
                      secure and deterministic.
                    </p>
                  </div>
                </div>

                <div className="flex gap-3">
                  <div className="flex-shrink-0 w-8 h-8 bg-primary/10 rounded-full flex items-center justify-center text-sm font-semibold">
                    3
                  </div>
                  <div>
                    <h4 className="font-semibold mb-1">
                      Extract Random Number
                    </h4>
                    <p className="text-sm text-muted-foreground">
                      We take the first 2 bytes of the hash and convert them to
                      a number between 0-65535. This gives us a fair,
                      unpredictable random number.
                    </p>
                  </div>
                </div>

                <div className="flex gap-3">
                  <div className="flex-shrink-0 w-8 h-8 bg-primary/10 rounded-full flex items-center justify-center text-sm font-semibold">
                    4
                  </div>
                  <div>
                    <h4 className="font-semibold mb-1">Calculate Target</h4>
                    <p className="text-sm text-muted-foreground">
                      Based on your multiplier, we calculate the target number:
                      {multiplier && (
                        <code className="block mt-1 p-2 bg-card/50 rounded text-xs">
                          Target = 65536 × 10 ÷ {multiplier} ={" "}
                          {Math.floor((65536 * 10) / parseFloat(multiplier))}
                        </code>
                      )}
                    </p>
                  </div>
                </div>

                <div className="flex gap-3">
                  <div className="flex-shrink-0 w-8 h-8 bg-primary/10 rounded-full flex items-center justify-center text-sm font-semibold">
                    5
                  </div>
                  <div>
                    <h4 className="font-semibold mb-1">Determine Winner</h4>
                    <p className="text-sm text-muted-foreground">
                      You win if the rolled number is less than the target
                      number. The lower the target, the higher the risk and
                      reward!
                    </p>
                  </div>
                </div>
              </div>

              <Alert className="bg-primary/5 border-primary/20">
                <AlertCircle className="h-4 w-4" />
                <AlertDescription className="text-sm">
                  This entire process is deterministic and verifiable. Given the
                  same inputs, you'll always get the same result, proving the
                  game is fair.
                </AlertDescription>
              </Alert>
            </CardContent>
          </Card>
        </div>

        {/* Verification Result */}
        {verificationResult && (
          <Card className="bg-gradient-to-br from-card to-card/50 border-primary/20">
            <CardHeader>
              <div className="flex items-center justify-between">
                <CardTitle className="flex items-center gap-2">
                  {verificationResult.error ? (
                    <AlertCircle className="w-5 h-5 text-destructive" />
                  ) : (
                    <CheckCircle2 className="w-5 h-5 text-green-500" />
                  )}
                  Verification Result
                </CardTitle>
                {!verificationResult.error && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={shareResult}
                    className="flex items-center gap-2"
                  >
                    <Share2 className="w-4 h-4" />
                    Share
                  </Button>
                )}
              </div>
            </CardHeader>
            <CardContent className="space-y-4">
              {verificationResult.error ? (
                <Alert variant="destructive">
                  <AlertCircle className="h-4 w-4" />
                  <AlertDescription>
                    {verificationResult.error}
                  </AlertDescription>
                </Alert>
              ) : (
                <div className="space-y-6">
                  {/* Result Summary */}
                  <div className="text-center">
                    <div
                      className={`text-3xl font-bold mb-2 ${
                        verificationResult.isWin
                          ? "text-green-500"
                          : "text-red-500"
                      }`}
                    >
                      {verificationResult.isWin ? "🎉 YOU WON!" : "😔 YOU LOST"}
                    </div>
                    <p className="text-muted-foreground">
                      Rolled {verificationResult.rolledNumber}, needed{" "}
                      {verificationResult.targetNumber} or lower
                    </p>
                  </div>

                  {/* Game Details */}
                  <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                    <div className="text-center p-4 bg-card/50 rounded-lg">
                      <div className="text-2xl font-bold text-primary">
                        {verificationResult.rolledNumber}
                      </div>
                      <div className="text-sm text-muted-foreground">
                        Rolled Number
                      </div>
                    </div>

                    <div className="text-center p-4 bg-card/50 rounded-lg">
                      <div className="text-2xl font-bold text-primary">
                        {verificationResult.targetNumber}
                      </div>
                      <div className="text-sm text-muted-foreground">
                        Target (Max)
                      </div>
                    </div>

                    <div className="text-center p-4 bg-card/50 rounded-lg">
                      <div className="text-2xl font-bold text-primary">
                        {verificationResult.multiplier}x
                      </div>
                      <div className="text-sm text-muted-foreground">
                        Multiplier
                      </div>
                    </div>

                    <div className="text-center p-4 bg-card/50 rounded-lg">
                      <div className="text-2xl font-bold text-green-500">
                        {verificationResult.winChance.toFixed(1)}%
                      </div>
                      <div className="text-sm text-muted-foreground">
                        Win Chance
                      </div>
                    </div>
                  </div>

                  {/* Technical Details */}
                  <div className="space-y-4">
                    <h3 className="font-semibold">Technical Details</h3>
                    <div className="space-y-2 font-mono text-sm">
                      <div>
                        <span className="text-muted-foreground">
                          Hash Input:
                        </span>
                        <div className="bg-card/50 p-2 rounded border break-all">
                          {nonce}
                          {txHash}
                        </div>
                      </div>
                      <div>
                        <span className="text-muted-foreground">
                          SHA256 Hash:
                        </span>
                        <div className="bg-card/50 p-2 rounded border break-all">
                          {verificationResult.hashHex}
                        </div>
                      </div>
                      <div>
                        <span className="text-muted-foreground">
                          First 2 bytes:
                        </span>
                        <div className="bg-card/50 p-2 rounded border">
                          {verificationResult.hashHex.substring(0, 4)} →{" "}
                          {verificationResult.rolledNumber}
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </CardContent>
          </Card>
        )}

        {/* How It Works */}
        <Card className="bg-gradient-to-br from-card to-card/50 border-primary/20">
          <CardHeader>
            <CardTitle>How Game Verification Works</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4 text-sm text-muted-foreground">
            <div className="space-y-2">
              <p>
                <strong>1. Hash Generation:</strong> We combine the nonce +
                transaction hash and create a SHA256 hash
              </p>
              <p>
                <strong>2. Random Number:</strong> We take the first 2 bytes of
                the hash as a big-endian 16-bit number (0-65535)
              </p>
              <p>
                <strong>3. Win Condition:</strong> You win if the rolled number
                is less than or equal to the target number
              </p>
              <p>
                <strong>4. Target Calculation:</strong> Target = (65536 × 1000)
                ÷ (multiplier × 100)
              </p>
            </div>

            <Alert>
              <AlertCircle className="h-4 w-4" />
              <AlertDescription>
                This verification process is identical to what happens on our
                server. The game outcome is determined by cryptographic hashing,
                making it impossible to manipulate.
              </AlertDescription>
            </Alert>
          </CardContent>
        </Card>
      </div>

      <Footer />
    </div>
  );
}
