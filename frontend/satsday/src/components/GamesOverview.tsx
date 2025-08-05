import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dice1, Coins, Spade, Target, Clock } from "lucide-react";
import { Link } from "react-router-dom";

export default function GamesOverview() {
  const games = [
    {
      id: "dice",
      name: "Dice",
      description:
        "Classic provably fair dice with 11 betting options and up to 1000x multipliers",
      icon: Dice1,
      status: "Available",
      multiplier: "1000x",
      link: "/game",
    },
    {
      id: "coin-flip",
      name: "Coin Flip",
      description: "Simple 50/50 heads or tails with instant results",
      icon: Coins,
      status: "Coming Soon",
      multiplier: "2x",
      link: "#",
    },
    {
      id: "blackjack",
      name: "Blackjack",
      description: "Classic card game with provably fair shuffling",
      icon: Spade,
      status: "Coming Soon",
      multiplier: "3.5x",
      link: "#",
    },
    {
      id: "roulette",
      name: "Roulette",
      description: "European roulette with transparent wheel spinning",
      icon: Target,
      status: "Coming Soon",
      multiplier: "36x",
      link: "#",
    },
    {
      id: "crash",
      name: "Crash",
      description: "Watch the multiplier grow and cash out before it crashes",
      icon: Clock,
      status: "Coming Soon",
      multiplier: "∞x",
      link: "#",
    },
  ];

  return (
    <section id="game-selection" className="py-20 px-6">
      <div className="max-w-7xl mx-auto">
        <div className="text-center mb-16">
          <h2 className="text-4xl md:text-5xl font-bold mb-6">
            <span className="bg-gradient-to-r from-primary to-orange-500 bg-clip-text text-transparent">
              Game Selection
            </span>
          </h2>
          <p className="text-xl text-muted-foreground max-w-2xl mx-auto">
            Choose from our collection of provably fair games, each powered by
            Bitcoin transactions
          </p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-8">
          {games.map((game) => {
            const IconComponent = game.icon;
            const isAvailable = game.status === "Available";

            return (
              <Card
                key={game.id}
                className={`relative overflow-hidden transition-all duration-300 hover:scale-105 ${
                  isAvailable
                    ? "bg-gradient-to-br from-card to-card/50 border-primary/20 hover:border-primary/40"
                    : "bg-gradient-to-br from-card/50 to-card/20 border-border/20"
                }`}
              >
                <CardHeader className="pb-4">
                  <div className="flex items-center justify-between">
                    <div
                      className={`w-12 h-12 rounded-lg flex items-center justify-center ${
                        isAvailable ? "bg-primary/10" : "bg-muted/50"
                      }`}
                    >
                      <IconComponent
                        className={`w-6 h-6 ${
                          isAvailable ? "text-primary" : "text-muted-foreground"
                        }`}
                      />
                    </div>
                    <div className="text-right">
                      <div className="text-2xl font-bold text-primary">
                        {game.multiplier}
                      </div>
                      <div className="text-xs text-muted-foreground">
                        Max Win
                      </div>
                    </div>
                  </div>
                  <CardTitle className="text-xl">{game.name}</CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                  <p className="text-muted-foreground text-sm leading-relaxed">
                    {game.description}
                  </p>

                  <div className="flex items-center justify-between">
                    <span
                      className={`px-3 py-1 rounded-full text-xs font-medium ${
                        isAvailable
                          ? "bg-green-500/10 text-green-400 border border-green-500/20"
                          : "bg-muted/50 text-muted-foreground border border-border/50"
                      }`}
                    >
                      {game.status}
                    </span>

                    {isAvailable ? (
                      <Button variant="game" size="sm" asChild>
                        <Link to={game.link}>Play Now</Link>
                      </Button>
                    ) : (
                      <Button variant="outline" size="sm" disabled>
                        Coming Soon
                      </Button>
                    )}
                  </div>
                </CardContent>

                {!isAvailable && (
                  <div className="absolute inset-0 bg-background/5 backdrop-blur-[1px]" />
                )}
              </Card>
            );
          })}
        </div>
      </div>
    </section>
  );
}
