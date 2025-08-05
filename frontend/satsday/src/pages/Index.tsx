import Navbar from "@/components/Navbar";
import GameHero from "@/components/GameHero";
import GamesOverview from "@/components/GamesOverview";

const Index = () => {
  return (
    <div className="min-h-screen bg-background">
      <Navbar />

      {/* Hero Section */}
      <GameHero />

      {/* Games Overview Section */}
      <section className="border-t border-border/50">
        <GamesOverview />
      </section>
    </div>
  );
};

export default Index;
