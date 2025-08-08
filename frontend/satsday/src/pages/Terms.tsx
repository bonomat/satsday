import Navbar from "@/components/Navbar";
import Footer from "@/components/Footer";
import { Link } from "react-router-dom";
import { ArrowLeft } from "lucide-react";

const Terms = () => {
  return (
    <div className="min-h-screen bg-background flex flex-col">
      <Navbar />

      <main className="flex-grow container mx-auto px-4 py-8 max-w-4xl">
        <Link
          to="/"
          className="inline-flex items-center gap-2 text-muted-foreground hover:text-foreground transition-colors mb-6"
        >
          <ArrowLeft className="h-4 w-4" />
          Back to Game
        </Link>

        <h1 className="text-4xl font-bold mb-8">Terms of Service</h1>

        <div className="prose prose-neutral dark:prose-invert max-w-none space-y-6">
          <section>
            <h2 className="text-2xl font-semibold mb-4">
              1. Acceptance of Terms
            </h2>
            <p className="text-muted-foreground">
              By accessing and using SatsDay (the "Service"), you acknowledge
              that you have read, understood, and agree to be bound by these
              Terms of Service. If you do not agree with these terms, you must
              not use the Service.
            </p>
          </section>

          <section>
            <h2 className="text-2xl font-semibold mb-4">
              2. How the Game Works
            </h2>
            <p className="text-muted-foreground mb-4">
              SatsDay is a provably fair dice game built on the Bitcoin
              Lightning Network using Ark protocol:
            </p>
            <ul className="list-disc pl-6 space-y-2 text-muted-foreground">
              <li>Players send Bitcoin to one of our game addresses</li>
              <li>
                Each address has a specific multiplier and win probability
              </li>
              <li>
                A random number between 0 and 65,535 is generated using a
                combination of your transaction ID and our daily server nonce
              </li>
              <li>
                If the number is below the target threshold for your chosen
                multiplier, you win
              </li>
              <li>
                Winners receive their bet amount multiplied by the chosen
                multiplier
              </li>
              <li>
                All results are cryptographically verifiable using the provided
                nonce after each day
              </li>
            </ul>
          </section>

          <section>
            <h2 className="text-2xl font-semibold mb-4">3. Risk Warning</h2>
            <div className="bg-destructive/10 border border-destructive/20 rounded-lg p-4 mb-4">
              <p className="font-semibold text-destructive mb-2">
                ⚠️ IMPORTANT GAMBLING RISK WARNING
              </p>
              <ul className="list-disc pl-6 space-y-2 text-muted-foreground">
                <li>
                  Gambling is inherently risky and you may lose all funds
                  wagered
                </li>
                <li>The house always has a mathematical edge</li>
                <li>Past results do not guarantee future outcomes</li>
                <li>Never gamble with money you cannot afford to lose</li>
                <li>Gambling can be addictive - please play responsibly</li>
              </ul>
            </div>
          </section>

          <section>
            <h2 className="text-2xl font-semibold mb-4">4. No Guarantees</h2>
            <p className="text-muted-foreground">
              The Service is provided "as is" without any warranties or
              guarantees of any kind, either express or implied. We do not
              guarantee:
            </p>
            <ul className="list-disc pl-6 space-y-2 text-muted-foreground mt-4">
              <li>
                Continuous, uninterrupted, or error-free operation of the
                Service
              </li>
              <li>The accuracy or reliability of any information provided</li>
              <li>That you will win any bets or make any profit</li>
              <li>The availability of funds for payouts at all times</li>
            </ul>
          </section>

          <section>
            <h2 className="text-2xl font-semibold mb-4">
              5. Service Modifications
            </h2>
            <p className="text-muted-foreground">
              We reserve the right to modify, suspend, or discontinue any aspect
              of the Service at any time without prior notice. This includes but
              is not limited to:
            </p>
            <ul className="list-disc pl-6 space-y-2 text-muted-foreground mt-4">
              <li>Game rules and multipliers</li>
              <li>Minimum and maximum bet amounts</li>
              <li>Payout procedures and timing</li>
              <li>Service availability</li>
            </ul>
          </section>

          <section>
            <h2 className="text-2xl font-semibold mb-4">
              6. Age and Jurisdiction Requirements
            </h2>
            <p className="text-muted-foreground">
              You must be at least 18 years old (or the legal gambling age in
              your jurisdiction, whichever is higher) to use this Service. It is
              your responsibility to ensure that online gambling is legal in
              your jurisdiction. We do not accept users from jurisdictions where
              online gambling is prohibited.
            </p>
          </section>

          <section>
            <h2 className="text-2xl font-semibold mb-4">
              7. Responsible Gaming
            </h2>
            <p className="text-muted-foreground">
              We encourage responsible gaming. If you feel you may have a
              gambling problem, please seek help:
            </p>
            <ul className="list-disc pl-6 space-y-2 text-muted-foreground mt-4">
              <li>Set limits on your gambling time and money</li>
              <li>Never chase losses</li>
              <li>Take regular breaks</li>
              <li>
                Seek professional help if gambling negatively affects your life
              </li>
            </ul>
          </section>

          <section>
            <h2 className="text-2xl font-semibold mb-4">
              8. Limitation of Liability
            </h2>
            <p className="text-muted-foreground">
              To the maximum extent permitted by law, we shall not be liable for
              any direct, indirect, incidental, special, consequential, or
              punitive damages resulting from your use or inability to use the
              Service, including but not limited to losses from gambling
              activities.
            </p>
          </section>

          <section>
            <h2 className="text-2xl font-semibold mb-4">
              9. Provably Fair Gaming
            </h2>
            <p className="text-muted-foreground">
              All game results are determined using a provably fair algorithm.
              The daily server nonce is revealed after rotation, allowing you to
              verify all previous game results. However, this does not guarantee
              winning outcomes or alter the inherent risks of gambling.
            </p>
          </section>

          <section>
            <h2 className="text-2xl font-semibold mb-4">
              10. Changes to Terms
            </h2>
            <p className="text-muted-foreground">
              We reserve the right to update these Terms of Service at any time
              without prior notice. Continued use of the Service after any
              changes constitutes acceptance of the new terms. It is your
              responsibility to review these terms periodically.
            </p>
          </section>

          <section>
            <h2 className="text-2xl font-semibold mb-4">
              11. Contact Information
            </h2>
            <p className="text-muted-foreground">
              For questions about these Terms of Service, please contact us via
              Nostr at{" "}
              <a
                href="https://njump.me/nprofile1qyn8wumn8ghj7en5wqhxsctvd9nxz7pwwfmhg6pdv9skx6r9dchxgef0dehhxarjqyd8wumn8ghj7un9d3shjtnwdaehgunrdpjkx6ewd4jj7qpqdk4qs4a3natms30s3gk2kfecrwpxev8ee0mcwlldrrkj73m6vf0q5365mp"
                target="_blank"
                className="text-primary hover:underline break-all"
              >
                npub1dk4qs4a3natms30s3gk2kfecrwpxev8ee0mcwlldrrkj73m6vf0q8q59r2
              </a>
            </p>
          </section>

          <section className="pt-8 border-t border-border/50">
            <p className="text-sm text-muted-foreground">
              Last updated: {new Date().toLocaleDateString()}
            </p>
          </section>
        </div>
      </main>

      <Footer />
    </div>
  );
};

export default Terms;
