import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Bitcoin, Shield, TrendingUp, Zap } from "lucide-react";

export default function GameHero() {
  return (
    <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-background via-background to-accent/5">
      <div className="max-w-6xl mx-auto px-6 text-center space-y-12">
        {/* Main Hero Content */}
        <div className="space-y-6">
          <h1 className="text-5xl md:text-7xl font-bold tracking-tight">
            <span className="bg-gradient-to-r from-primary via-orange-500 to-yellow-500 bg-clip-text text-transparent">
              Every day is Satsday
            </span>
            <br />
            <span className="text-foreground text-4xl">
              Provably Fair Bitcoin Games
            </span>
          </h1>

          <p className="text-xl md:text-2xl text-muted-foreground max-w-3xl mx-auto leading-relaxed">
            Experience transparent gaming powered by Ark. Every outcome is
            verifiable. Every game is provably fair. Every day is Satsday.
          </p>

          <div className="flex flex-col sm:flex-row gap-4 justify-center items-center">
            <Button
              variant="game"
              size="lg"
              className="text-lg px-8 py-6"
              onClick={() => {
                const gameSection = document.getElementById("game-selection");
                gameSection?.scrollIntoView({ behavior: "smooth" });
              }}
            >
              <Zap className="w-5 h-5 mr-2" />
              Explore Games
            </Button>
          </div>
        </div>

        {/* Feature Cards */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6 mt-16">
          <Card className="bg-gradient-to-br from-card to-card/50 border-primary/20 hover:border-primary/40 transition-all duration-300">
            <CardContent className="pt-6 pb-6 text-center space-y-4">
              <div className="w-12 h-12 bg-primary/10 rounded-lg flex items-center justify-center mx-auto">
                <Shield className="w-6 h-6 text-primary" />
              </div>
              <h3 className="text-xl font-semibold">Provably Fair</h3>
              <p className="text-muted-foreground">
                Every game outcome is verifiable using Bitcoin transaction
                hashes and cryptographic proofs.
              </p>
            </CardContent>
          </Card>

          <Card className="bg-gradient-to-br from-card to-card/50 border-primary/20 hover:border-primary/40 transition-all duration-300">
            <CardContent className="pt-6 pb-6 text-center space-y-4">
              <div className="w-12 h-12 bg-primary/10 rounded-lg flex items-center justify-center mx-auto">
                <Bitcoin className="w-6 h-6 text-primary" />
              </div>
              <h3 className="text-xl font-semibold">Ark Settlement</h3>
              <p className="text-muted-foreground">
                Fast and efficient settlements using Ark layer while maintaining
                Bitcoin security.
              </p>
            </CardContent>
          </Card>

          <Card className="bg-gradient-to-br from-card to-card/50 border-primary/20 hover:border-primary/40 transition-all duration-300">
            <CardContent className="pt-6 pb-6 text-center space-y-4">
              <div className="w-12 h-12 bg-primary/10 rounded-lg flex items-center justify-center mx-auto">
                <TrendingUp className="w-6 h-6 text-primary" />
              </div>
              <h3 className="text-xl font-semibold">Multiple Games</h3>
              <p className="text-muted-foreground">
                Various game types with different strategies and payout
                structures to choose from.
              </p>
            </CardContent>
          </Card>
        </div>

        {/* Stats */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-8 pt-8 border-t border-border/50">
          <div className="text-center">
            <div className="text-3xl font-bold text-primary">5+</div>
            <div className="text-sm text-muted-foreground">Game Types</div>
          </div>
          <div className="text-center">
            <div className="text-3xl font-bold text-primary">1000x</div>
            <div className="text-sm text-muted-foreground">Max Multiplier</div>
          </div>
          <div className="text-center">
            <div className="text-3xl font-bold text-primary">100%</div>
            <div className="text-sm text-muted-foreground">Transparent</div>
          </div>
          <div className="text-center">
            <div className="text-3xl font-bold text-primary">∞</div>
            <div className="text-sm text-muted-foreground">Verifiable</div>
          </div>
        </div>
      </div>
    </div>
  );
}
